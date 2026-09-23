use std::sync::Arc;

use aicore_contracts::platform::service::{IServiceController, ServiceState};

/// 升级计划。
///
/// 步骤：
/// 1. 备份当前二进制（controller 负责具体备份）。
/// 2. 停止服务。
/// 3. controller.upgrade(new_binary) 覆盖二进制。
/// 4. 启动服务。
/// 5. 健康检查（query_state == Running）。
/// 6. 失败 → controller.rollback() → 启动。
pub struct UpgradePlan {
    controller: Arc<dyn IServiceController>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UpgradeOutcome {
    Succeeded,
    RolledBack { reason: String },
    Failed { reason: String },
}

impl UpgradePlan {
    pub fn new(controller: Arc<dyn IServiceController>) -> Self {
        Self { controller }
    }

    pub fn run(&self, new_binary_path: &str) -> UpgradeOutcome {
        // 1. 停服务
        let stop = self.controller.stop();
        if !stop.success && self.controller.query_state() != ServiceState::Stopped {
            return UpgradeOutcome::Failed {
                reason: format!("stop failed: {:?}", stop.message),
            };
        }

        // 2. 升级
        let upgrade = self.controller.upgrade(new_binary_path);
        if !upgrade.success {
            return UpgradeOutcome::Failed {
                reason: format!("upgrade failed: {:?}", upgrade.message),
            };
        }

        // 3. 启动
        let start = self.controller.start();
        if !start.success {
            return self.rollback("start failed after upgrade");
        }

        // 4. 健康检查
        match self.health_check() {
            Ok(()) => UpgradeOutcome::Succeeded,
            Err(reason) => self.rollback(&reason),
        }
    }

    fn health_check(&self) -> Result<(), String> {
        for _ in 0..30 {
            std::thread::sleep(std::time::Duration::from_millis(200));
            let s = self.controller.query_state();
            if s == ServiceState::Running {
                return Ok(());
            }
            if s == ServiceState::Failed {
                return Err("service entered failed state".to_string());
            }
        }
        Err("health check timeout".to_string())
    }

    fn rollback(&self, reason: &str) -> UpgradeOutcome {
        let _ = self.controller.stop();
        let rollback = self.controller.rollback();
        if !rollback.success {
            return UpgradeOutcome::Failed {
                reason: format!("{}; rollback also failed: {:?}", reason, rollback.message),
            };
        }
        let start = self.controller.start();
        if !start.success {
            return UpgradeOutcome::Failed {
                reason: format!("{}; restart after rollback failed", reason),
            };
        }
        UpgradeOutcome::RolledBack {
            reason: reason.to_string(),
        }
    }
}
