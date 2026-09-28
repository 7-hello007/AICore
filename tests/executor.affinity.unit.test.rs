//! Linux CPU testing.
//!
//! Target: Verify the real `sched_setaffinity`/ `sched_getaffinity` path.
//! Permission limited only to verify the wrong path; not Linux skip.

#[cfg(target_os = "linux")]
use aicore_adapter_linux::LinuxProcessController;
#[cfg(target_os = "linux")]
use aicore_contracts::traits::process_controller::{AffinityMask, IProcessController};

#[cfg(target_os = "linux")]
#[test]
fn get_affinity_for_self_returns_at_least_one_core() {
    let c = LinuxProcessController::new();
    let pid = std::process::id();
    let mask = c.get_affinity(pid).expect("get_affinity on self");
    assert!(!mask.cores.is_empty(), "self must run on at least one core");
}

#[cfg(target_os = "linux")]
#[test]
fn set_affinity_for_self_succeeds() {
    let c = LinuxProcessController::new();
    let pid = std::process::id();

    // Read the original first
    let original = c.get_affinity(pid).expect("get original");

    // Set to original mask (Equivalent) with success
    c.set_affinity(pid, &original).expect("set to original");

    // Single nuclear test (if original mask is multiple)
    if original.cores.len() > 1 {
        let single = AffinityMask::new(vec![original.cores[0]]);
        c.set_affinity(pid, &single).expect("set to single core");

        let after = c.get_affinity(pid).expect("get after single");
        assert_eq!(after.cores, vec![original.cores[0]]);

        // Restore
        c.set_affinity(pid, &original).expect("restore");
    }
}

#[cfg(target_os = "linux")]
#[test]
fn set_affinity_rejects_empty_mask() {
    let c = LinuxProcessController::new();
    let pid = std::process::id();
    let err = c.set_affinity(pid, &AffinityMask::new(vec![])).unwrap_err();
    let s = format!("{}", err);
    assert!(s.contains("ERR_INVALID_REQUEST"), "got: {}", s);
}

#[cfg(target_os = "linux")]
#[test]
fn set_affinity_rejects_out_of_range_core() {
    let c = LinuxProcessController::new();
    let pid = std::process::id();
    let err = c
        .set_affinity(pid, &AffinityMask::new(vec![999_999]))
        .unwrap_err();
    let s = format!("{}", err);
    assert!(s.contains("ERR_INVALID_REQUEST"), "got: {}", s);
}

#[cfg(target_os = "linux")]
#[test]
fn linux_declares_affinity_capabilities() {
    let c = LinuxProcessController::new();
    let caps = c.capabilities();
    assert!(caps.contains(&"process.affinity.read"));
    assert!(caps.contains(&"process.affinity.write"));
}
