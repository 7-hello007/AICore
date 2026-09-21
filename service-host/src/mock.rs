use std::sync::Mutex;

use aicore_contracts::platform::service::{
    IServiceController, ServiceConfig, ServiceOperationResult, ServiceState,
};

/// Mock service controller（测试用）。
///
/// 状态机：
/// - NotInstalled → Stopped（install）
/// - Stopped → Running（start）
/// - Running → Stopped（stop）
/// - upgrade：模拟二进制替换；可配置是否失败
/// - rollback：恢复上一个 binary path
pub struct MockServiceController {
    state: Mutex<MockState>,
    /// 下一次 upgrade 是否失败
    fail_next_upgrade: Mutex<bool>,
    /// 下一次 start 是否失败
    fail_next_start: Mutex<bool>,
    /// 备份的 binary path 栈
    backups: Mutex<Vec<String>>,
    current_binary: Mutex<String>,
}

#[derive(Debug, Clone)]
struct MockState {
    state: ServiceState,
}

impl MockServiceController {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(MockState {
                state: ServiceState::NotInstalled,
            }),
            fail_next_upgrade: Mutex::new(false),
            fail_next_start: Mutex::new(false),
            backups: Mutex::new(Vec::new()),
            current_binary: Mutex::new("/mock/bin/aicored".to_string()),
        }
    }

    pub fn with_failing_upgrade(self) -> Self {
        *self.fail_next_upgrade.lock().unwrap() = true;
        self
    }

    pub fn with_failing_start(self) -> Self {
        *self.fail_next_start.lock().unwrap() = true;
        self
    }

    pub fn current_binary(&self) -> String {
        self.current_binary.lock().unwrap().clone()
    }

    pub fn backup_count(&self) -> usize {
        self.backups.lock().unwrap().len()
    }
}

impl Default for MockServiceController {
    fn default() -> Self {
        Self::new()
    }
}

impl IServiceController for MockServiceController {
    fn install(&self, _config: &ServiceConfig) -> ServiceOperationResult {
        let mut s = self.state.lock().unwrap();
        s.state = ServiceState::Stopped;
        ServiceOperationResult::ok(ServiceState::Stopped)
    }

    fn uninstall(&self) -> ServiceOperationResult {
        let mut s = self.state.lock().unwrap();
        s.state = ServiceState::NotInstalled;
        ServiceOperationResult::ok(ServiceState::NotInstalled)
    }

    fn start(&self) -> ServiceOperationResult {
        if *self.fail_next_start.lock().unwrap() {
            *self.fail_next_start.lock().unwrap() = false;
            let mut s = self.state.lock().unwrap();
            s.state = ServiceState::Failed;
            return ServiceOperationResult::failed(ServiceState::Failed, "mock start failure");
        }
        let mut s = self.state.lock().unwrap();
        s.state = ServiceState::Running;
        ServiceOperationResult::ok(ServiceState::Running)
    }

    fn stop(&self) -> ServiceOperationResult {
        let mut s = self.state.lock().unwrap();
        s.state = ServiceState::Stopped;
        ServiceOperationResult::ok(ServiceState::Stopped)
    }

    fn upgrade(&self, new_binary_path: &str) -> ServiceOperationResult {
        if *self.fail_next_upgrade.lock().unwrap() {
            *self.fail_next_upgrade.lock().unwrap() = false;
            return ServiceOperationResult::failed(ServiceState::Stopped, "mock upgrade failure");
        }
        // 备份当前 binary
        let current = self.current_binary.lock().unwrap().clone();
        self.backups.lock().unwrap().push(current);
        *self.current_binary.lock().unwrap() = new_binary_path.to_string();
        ServiceOperationResult::ok(ServiceState::Stopped)
    }

    fn rollback(&self) -> ServiceOperationResult {
        let mut backups = self.backups.lock().unwrap();
        match backups.pop() {
            Some(prev) => {
                *self.current_binary.lock().unwrap() = prev;
                ServiceOperationResult::ok(ServiceState::Stopped)
            },
            None => ServiceOperationResult::failed(ServiceState::Stopped, "no backup to rollback"),
        }
    }

    fn query_state(&self) -> ServiceState {
        self.state.lock().unwrap().state
    }

    fn controller_name(&self) -> &'static str {
        "mock_service_controller"
    }
}
