#[cfg(target_os = "windows")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use aicore_adapter_windows::WindowsPowerController;
    use aicore_contracts::traits::power_controller::{IPowerController, PowerStateSnapshot};
    struct Restore {
        controller: WindowsPowerController,
        original: PowerStateSnapshot,
    }
    impl Drop for Restore {
        fn drop(&mut self) {
            eprintln!("NATIVE RESTORE device=power original_now={:?} proposed={:?} rollback=powercfg /setactive {}",self.controller.capture_state(),self.original,self.original.state_id);
            let result = self
                .controller
                .restore_state(&self.original)
                .and_then(|()| self.controller.capture_state());
            let ok = result.as_ref().is_ok_and(|s| s == &self.original);
            eprintln!(
                "RESTORATION {} device=power readback={result:?}",
                if ok { "PASS" } else { "FAIL" }
            );
            if !std::thread::panicking() {
                assert!(ok, "power restoration failed");
            }
        }
    }
    let controller = WindowsPowerController::new();
    let original = controller.capture_state()?;
    let target = PowerStateSnapshot {
        platform: "windows".into(),
        state_id: std::env::var("AICORE_POWER_PROBE_SCHEME")?,
    };
    if original == target {
        return Err("probe requires distinct pre-created owned clone GUID".into());
    }
    let guard = Restore {
        controller,
        original,
    };
    println!("NATIVE BEFORE device=power original={:?} proposed={target:?} rollback=powercfg /setactive {}",guard.original,guard.original.state_id);
    guard.controller.restore_state(&target)?;
    if guard.controller.capture_state()? != target {
        return Err("native power GUID readback mismatch".into());
    }
    println!("PASS native power changed to owned clone with exact readback");
    drop(guard);
    Ok(())
}
#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("NOT VERIFIED Windows power API on this OS");
}
