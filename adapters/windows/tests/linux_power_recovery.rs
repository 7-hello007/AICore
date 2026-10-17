//! Portable filesystem regression harness: never accesses host /sys or native setters.
#[path = "../../linux/src/power/controller.rs"]
mod linux_power;

use aicore_contracts::{
    policy::resource_policy::PowerMode, traits::power_controller::PowerStateSnapshot,
};
use linux_power::LinuxPowerController as Controller;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "aicore-linux-power-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
    fn governor(&self, cpu: u32) -> PathBuf {
        self.0.join(format!(
            "devices/system/cpu/cpu{cpu}/cpufreq/scaling_governor"
        ))
    }
    fn cpu(&self, cpu: u32, value: &str) {
        let path = self.governor(cpu);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, value).unwrap();
    }
    fn profile(&self) -> PathBuf {
        self.0.join("firmware/acpi/platform_profile")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap().trim().into()
}

#[test]
fn mixed_governors_do_not_claim_cpu0_mode() {
    let f = Fixture::new();
    f.cpu(0, "performance");
    f.cpu(1, "powersave");
    assert!(Controller::get_profile_at(&f.0).is_err());
}

#[test]
fn custom_and_mixed_governors_restore_exactly() {
    let f = Fixture::new();
    f.cpu(0, "ondemand");
    f.cpu(2, "vendor-custom");
    let state = Controller::capture_state_at(&f.0).unwrap();
    Controller::set_profile_at(&f.0, PowerMode::Performance).unwrap();
    Controller::restore_state_at(&f.0, &state).unwrap();
    assert_eq!(read(&f.governor(0)), "ondemand");
    assert_eq!(read(&f.governor(2)), "vendor-custom");
}

#[test]
fn custom_platform_profile_restores_exactly() {
    let f = Fixture::new();
    fs::create_dir_all(f.profile().parent().unwrap()).unwrap();
    fs::write(f.profile(), "vendor-custom\n").unwrap();
    let state = Controller::capture_state_at(&f.0).unwrap();
    Controller::set_profile_at(&f.0, PowerMode::Performance).unwrap();
    Controller::restore_state_at(&f.0, &state).unwrap();
    assert_eq!(read(&f.profile()), "vendor-custom");
}

#[test]
fn failed_apply_recovers_preceding_cpu() {
    let f = Fixture::new();
    f.cpu(0, "powersave");
    f.cpu(1, "schedutil");
    let original_permissions = fs::metadata(f.governor(1)).unwrap().permissions();
    let mut permissions = original_permissions.clone();
    permissions.set_readonly(true);
    fs::set_permissions(f.governor(1), permissions).unwrap();
    let result = Controller::set_profile_at(&f.0, PowerMode::Performance);
    fs::set_permissions(f.governor(1), original_permissions).unwrap();
    assert!(result.is_err());
    assert_eq!(read(&f.governor(0)), "powersave");
}

#[test]
fn restore_attempts_other_cpus_after_first_failure() {
    let f = Fixture::new();
    f.cpu(0, "performance");
    f.cpu(1, "performance");
    let state = Controller::capture_state_at(&f.0).unwrap();
    f.cpu(0, "powersave");
    f.cpu(1, "powersave");
    fs::remove_file(f.governor(0)).unwrap();
    fs::create_dir(f.governor(0)).unwrap();
    assert!(Controller::restore_state_at(&f.0, &state).is_err());
    assert_eq!(read(&f.governor(1)), "performance");
}

#[test]
fn invalid_snapshot_cannot_redirect_writes() {
    let f = Fixture::new();
    f.cpu(0, "powersave");
    for state in [
        PowerStateSnapshot {
            platform: "windows".into(),
            state_id: "performance".into(),
        },
        PowerStateSnapshot {
            platform: "linux".into(),
            state_id: "governors-v1\n../outside\tperformance".into(),
        },
    ] {
        assert!(Controller::restore_state_at(&f.0, &state).is_err());
        assert_eq!(read(&f.governor(0)), "powersave");
    }
}

#[allow(dead_code)]
#[path = "../../linux/src/process/controller.rs"]
mod linux_process;

#[test]
fn priority_lowering_is_refused_without_reverse_authority() {
    assert!(linux_process::validate_nice_change(-3, 10).is_err());
    assert!(linux_process::validate_nice_change(0, 19).is_err());
    assert!(linux_process::validate_nice_change(7, 7).is_ok());
    assert!(linux_process::validate_nice_change(7, 0).is_ok());
}

#[test]
fn native_priority_snapshot_preserves_values_between_policy_buckets() {
    for nice in [-20, -9, -3, 4, 7, 18, 19] {
        let state = linux_process::encode_priority_snapshot(123, 456, nice);
        assert_eq!(
            linux_process::decode_priority_snapshot(&state).unwrap(),
            (456, nice)
        );
    }
}

#[test]
fn native_priority_snapshot_rejects_platform_pid_and_range_mismatch() {
    use aicore_contracts::traits::process_controller::ProcessPrioritySnapshot;
    for (platform, pid, state_id) in [
        ("windows", 123, "nice-v1:123:456:0"),
        ("linux", 124, "nice-v1:123:456:0"),
        ("linux", 0, "nice-v1:0:456:0"),
        ("linux", 123, "nice-v1:123:0:0"),
        ("linux", 123, "nice-v1:123:456:20"),
        ("linux", 123, "nice-v1:123:456:-21"),
        ("linux", 123, "nice-v1:123:456:0:extra"),
    ] {
        let state = ProcessPrioritySnapshot {
            platform: platform.into(),
            pid,
            state_id: state_id.into(),
        };
        assert!(linux_process::decode_priority_snapshot(&state).is_err());
    }
}

#[test]
fn missing_governor_is_not_recreated_and_other_cpus_restore() {
    let f = Fixture::new();
    f.cpu(0, "ondemand");
    f.cpu(1, "schedutil");
    let state = Controller::capture_state_at(&f.0).unwrap();
    fs::remove_file(f.governor(0)).unwrap();
    f.cpu(1, "performance");
    assert!(Controller::restore_state_at(&f.0, &state).is_err());
    assert!(!f.governor(0).exists());
    assert_eq!(read(&f.governor(1)), "schedutil");
}

#[test]
fn invalid_later_snapshot_entry_prevents_all_writes() {
    let f = Fixture::new();
    f.cpu(0, "powersave");
    for state_id in [
        "governors-v1\ncpu0\tperformance\ncpu0\tschedutil",
        "governors-v1\ncpu0\tperformance\n../../outside\tschedutil",
        "governors-v1\ncpu0\tperformance\ncpu1\tbad/value",
        "profile-v1\nperformance\nextra",
    ] {
        let state = PowerStateSnapshot {
            platform: "linux".into(),
            state_id: state_id.into(),
        };
        assert!(Controller::restore_state_at(&f.0, &state).is_err());
        assert_eq!(read(&f.governor(0)), "powersave");
    }
}

#[test]
fn process_lifetime_mismatch_is_refused() {
    assert!(linux_process::ensure_process_lifetime(456, 457).is_err());
    assert!(linux_process::ensure_process_lifetime(0, 0).is_err());
    assert!(linux_process::ensure_process_lifetime(456, 456).is_ok());
}

#[test]
fn process_stat_parser_handles_spaces_and_parentheses_in_name() {
    // Native fields 3..21 precede starttime (field 22).
    let stat = "123 (worker ) name) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 456 20 21";
    assert_eq!(linux_process::parse_process_start(123, stat).unwrap(), 456);
    assert!(linux_process::parse_process_start(124, stat).is_err());
    assert!(linux_process::parse_process_start(123, "123 (short) S 1").is_err());
}

#[test]
fn final_readback_detects_coupled_governor_mismatch() {
    let f = Fixture::new();
    f.cpu(0, "ondemand");
    f.cpu(1, "schedutil");
    f.cpu(2, "powersave");
    let state = Controller::capture_state_at(&f.0).unwrap();
    // Model a changed cpufreq policy association using linked temporary files.
    fs::remove_file(f.governor(1)).unwrap();
    fs::hard_link(f.governor(0), f.governor(1)).unwrap();
    f.cpu(0, "performance");
    f.cpu(2, "performance");
    assert!(Controller::restore_state_at(&f.0, &state).is_err());
    assert_eq!(read(&f.governor(2)), "powersave");
}

#[test]
fn priority_readback_compares_exact_native_value_and_lifetime() {
    let expected = linux_process::encode_priority_snapshot(123, 456, -3);
    for actual in [
        linux_process::encode_priority_snapshot(123, 456, 0),
        linux_process::encode_priority_snapshot(123, 457, -3),
        linux_process::encode_priority_snapshot(124, 456, -3),
    ] {
        assert!(linux_process::verify_priority_readback(&expected, &actual).is_err());
    }
    assert!(linux_process::verify_priority_readback(&expected, &expected).is_ok());
}
