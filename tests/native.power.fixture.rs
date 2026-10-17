#[cfg(target_os = "linux")]
use aicore_adapter_linux::LinuxPowerController as PowerController;
#[cfg(target_os = "windows")]
use aicore_adapter_windows::WindowsPowerController as PowerController;
use aicore_contracts::errors::ContractResult;
use aicore_contracts::{
    policy::resource_policy::PowerMode,
    traits::power_controller::{IPowerController, PowerStateSnapshot},
};
pub struct OwnedPowerSnapshot {
    controller: PowerController,
    original: PowerStateSnapshot,
}
impl OwnedPowerSnapshot {
    pub fn new() -> ContractResult<Self> {
        let controller = PowerController::new();
        let original = controller.capture_state()?;
        Ok(Self {
            controller,
            original,
        })
    }
}
impl IPowerController for OwnedPowerSnapshot {
    fn supports_exact_state(&self) -> bool {
        self.controller.supports_exact_state()
    }
    fn set_profile(&self, mode: PowerMode) -> ContractResult<PowerMode> {
        eprintln!("NATIVE BEFORE device=power original={:?} proposed={mode:?} rollback=restore_state({:?})",self.controller.capture_state()?,self.original);
        self.controller.set_profile(mode)
    }
    fn get_profile(&self) -> ContractResult<PowerMode> {
        self.controller.get_profile()
    }
    fn capture_state(&self) -> ContractResult<PowerStateSnapshot> {
        self.controller.capture_state()
    }
    fn restore_state(&self, state: &PowerStateSnapshot) -> ContractResult<()> {
        self.controller.restore_state(state)
    }
    fn capabilities(&self) -> Vec<&'static str> {
        self.controller.capabilities()
    }
    fn platform_name(&self) -> &'static str {
        self.controller.platform_name()
    }
}
impl Drop for OwnedPowerSnapshot {
    fn drop(&mut self) {
        eprintln!("NATIVE RESTORE device=power original_now={:?} proposed={:?} rollback=restore_state({:?})",self.controller.capture_state(),self.original,self.original);
        let restored = self
            .controller
            .restore_state(&self.original)
            .and_then(|()| self.controller.capture_state());
        let matches = restored.as_ref().is_ok_and(|s| s == &self.original);
        eprintln!(
            "RESTORATION {} device=power readback={restored:?}",
            if matches { "PASS" } else { "FAIL" }
        );
        if !std::thread::panicking() {
            assert!(matches, "power restoration failed");
        }
    }
}
