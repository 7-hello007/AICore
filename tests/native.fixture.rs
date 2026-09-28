//! Native tests may mutate only the child owned by this fixture.
#[cfg(target_os = "linux")]
use aicore_adapter_linux::LinuxProcessController as Controller;
#[cfg(target_os = "windows")]
use aicore_adapter_windows::WindowsProcessController as Controller;
use aicore_contracts::{
    errors::{ContractError, ContractResult},
    execution::targets::ExecutionTargets,
    policy::priority::PolicyPriority,
    traits::process_controller::{AffinityMask, IProcessController},
};
use std::{
    process::{Child, Command, Stdio},
    sync::Mutex,
};
pub struct OwnedProcess {
    child: Mutex<Child>,
    controller: Controller,
    priority: PolicyPriority,
    affinity: AffinityMask,
    closed: std::sync::atomic::AtomicBool,
}
impl OwnedProcess {
    pub fn new() -> Self {
        #[cfg(target_os = "windows")]
        let mut command = {
            use std::os::windows::process::CommandExt;
            let mut c = Command::new("powershell.exe");
            c.args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Seconds 300",
            ])
            .creation_flags(0x08000000);
            c
        };
        #[cfg(target_os = "linux")]
        let mut command = {
            let mut c = Command::new("sleep");
            c.arg("300");
            c
        };
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("owned child creation");
        let controller = Controller::new();
        let initial = controller
            .get_priority(child.id())
            .and_then(|p| controller.get_affinity(child.id()).map(|a| (p, a)));
        let (priority, affinity) = match initial {
            Ok(state) => state,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("owned child snapshot failed: {error}");
            },
        };
        if priority != PolicyPriority::Normal {
            let _ = child.kill();
            let _ = child.wait();
            panic!("native fixture requires a fresh Normal child for exact priority restoration");
        }
        Self {
            child: Mutex::new(child),
            controller,
            priority,
            affinity,
            closed: std::sync::atomic::AtomicBool::new(false),
        }
    }
    pub fn targets(&self) -> ExecutionTargets {
        ExecutionTargets {
            foreground_pids: vec![self.pid()],
            process_only: true,
            ..Default::default()
        }
    }
    fn pid(&self) -> u32 {
        self.child.lock().expect("owned child lock").id()
    }
    fn validate(&self, pid: u32) -> ContractResult<()> {
        if self.closed.load(std::sync::atomic::Ordering::Acquire) {
            return Err(ContractError::ResourceUnavailable(
                "owned child already closed".into(),
            ));
        }
        if pid != self.pid() {
            return Err(ContractError::PermissionDenied(
                "native fixture refuses unowned PID".into(),
            ));
        }
        Ok(())
    }
    pub fn finish(&self) -> ContractResult<()> {
        if self.closed.load(std::sync::atomic::Ordering::Acquire) {
            return Ok(());
        }
        let restored = self.restore();
        let mut child = self
            .child
            .lock()
            .map_err(|_| ContractError::LockPoisoned("owned child lock".into()))?;
        let _ = child.kill();
        let _ = child.wait();
        self.closed
            .store(true, std::sync::atomic::Ordering::Release);
        restored
    }
    pub fn restore(&self) -> ContractResult<()> {
        let pid = self.pid();
        eprintln!(
            "NATIVE RESTORE pid={pid} original_now={:?}/{:?} proposed={:?}/{:?} rollback=restore_original_snapshot",
            self.controller.get_priority(pid)?,self.controller.get_affinity(pid)?,self.priority, self.affinity
        );
        self.controller.set_priority(pid, self.priority)?;
        self.controller.set_affinity(pid, &self.affinity)?;
        if self.controller.get_priority(pid)? != self.priority
            || self.controller.get_affinity(pid)? != self.affinity
        {
            return Err(ContractError::AdapterUnavailable(
                "native restoration mismatch".into(),
            ));
        }
        eprintln!("RESTORATION PASS pid={pid} priority and affinity readback match original");
        Ok(())
    }
}
impl IProcessController for OwnedProcess {
    fn set_priority(&self, pid: u32, value: PolicyPriority) -> ContractResult<()> {
        self.validate(pid)?;
        eprintln!("NATIVE BEFORE pid={pid} original={:?} proposed={value:?} rollback=IProcessController::set_priority({pid}, {:?})",self.controller.get_priority(pid)?,self.priority);
        self.controller.set_priority(pid, value)
    }
    fn get_priority(&self, pid: u32) -> ContractResult<PolicyPriority> {
        self.validate(pid)?;
        self.controller.get_priority(pid)
    }
    fn set_affinity(&self, pid: u32, value: &AffinityMask) -> ContractResult<()> {
        self.validate(pid)?;
        eprintln!("NATIVE BEFORE pid={pid} original={:?} proposed={value:?} rollback=IProcessController::set_affinity({pid}, {:?})",self.controller.get_affinity(pid)?,self.affinity);
        self.controller.set_affinity(pid, value)
    }
    fn get_affinity(&self, pid: u32) -> ContractResult<AffinityMask> {
        self.validate(pid)?;
        self.controller.get_affinity(pid)
    }
    fn capabilities(&self) -> Vec<&'static str> {
        self.controller.capabilities()
    }
    fn platform_name(&self) -> &'static str {
        self.controller.platform_name()
    }
}
impl Drop for OwnedProcess {
    fn drop(&mut self) {
        let restored = self.finish();
        if let Err(error) = &restored {
            eprintln!("RESTORATION FAIL pid={} error={error}", self.pid());
        }
        if !std::thread::panicking() {
            assert!(
                restored.is_ok(),
                "owned process state restoration failed: {restored:?}"
            );
        }
    }
}
