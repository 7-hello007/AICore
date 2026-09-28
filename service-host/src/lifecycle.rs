use aicore_contracts::platform::service::{
    IServiceController, ServiceConfig, ServiceOperationResult, ServiceState,
};
use std::sync::Arc;

/// The Platform ' s unrelated service life cycle organization.
///
/// Cover controll, provide:
/// - install (if not installed)
/// - ensure_started.
/// - ensure_stopped.
/// - health_check (survey queries until Running or timeout)
pub struct ServiceLifecycle {
    controller: Arc<dyn IServiceController>,
}

impl ServiceLifecycle {
    pub fn new(controller: Arc<dyn IServiceController>) -> Self {
        Self { controller }
    }

    pub fn controller_name(&self) -> &'static str {
        self.controller.controller_name()
    }

    pub fn current_state(&self) -> ServiceState {
        self.controller.query_state()
    }

    /// Installation services (if installed, etc.).
    pub fn install(&self, config: &ServiceConfig) -> ServiceOperationResult {
        if let Err(e) = config.validate() {
            return ServiceOperationResult::failed(ServiceState::Failed, e.to_string());
        }
        if self.controller.query_state() != ServiceState::NotInstalled {
            return ServiceOperationResult::ok(self.controller.query_state());
        }
        self.controller.install(config)
    }

    /// Wait and see.
    pub fn ensure_started(&self) -> ServiceOperationResult {
        match self.controller.query_state() {
            ServiceState::Running => ServiceOperationResult::ok(ServiceState::Running),
            ServiceState::Starting => self.wait_until(ServiceState::Running),
            ServiceState::NotInstalled => {
                ServiceOperationResult::failed(ServiceState::NotInstalled, "service not installed")
            },
            _ => {
                let start = self.controller.start();
                if !start.success {
                    return start;
                }
                self.wait_until(ServiceState::Running)
            },
        }
    }

    /// Wait a minute.
    pub fn ensure_stopped(&self) -> ServiceOperationResult {
        match self.controller.query_state() {
            ServiceState::Stopped => ServiceOperationResult::ok(ServiceState::Stopped),
            ServiceState::Stopping => self.wait_until(ServiceState::Stopped),
            ServiceState::NotInstalled => {
                ServiceOperationResult::failed(ServiceState::NotInstalled, "service not installed")
            },
            _ => {
                let stop = self.controller.stop();
                if !stop.success {
                    return stop;
                }
                self.wait_until(ServiceState::Stopped)
            },
        }
    }

    /// Unmount.
    pub fn uninstall(&self) -> ServiceOperationResult {
        if self.controller.query_state() == ServiceState::NotInstalled {
            return ServiceOperationResult::ok(ServiceState::NotInstalled);
        }
        let _ = self.controller.stop();
        self.controller.uninstall()
    }

    /// Round queries until target state or timeout (~10s).
    fn wait_until(&self, target: ServiceState) -> ServiceOperationResult {
        for _ in 0..50 {
            std::thread::sleep(std::time::Duration::from_millis(200));
            let s = self.controller.query_state();
            if s == target {
                return ServiceOperationResult::ok(s);
            }
            if s == ServiceState::Failed {
                return ServiceOperationResult::failed(s, "service entered failed state");
            }
        }
        ServiceOperationResult::failed(
            self.controller.query_state(),
            format!("timeout waiting for {:?}", target),
        )
    }
}
