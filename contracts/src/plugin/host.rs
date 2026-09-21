use serde::{Deserialize, Serialize};

/// 插件进程状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginHostState {
    NotStarted,
    Starting,
    Running,
    Stopping,
    Stopped,
    Crashed,
    Failed,
}

impl PluginHostState {
    pub fn is_operational(self) -> bool {
        matches!(self, Self::Running)
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Stopped | Self::Crashed | Self::Failed)
    }
}

/// 插件进程规格。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginProcessSpec {
    pub plugin_id: String,
    pub artifact_path: String,
    /// 最大执行时间（毫秒）。超时后 host 终止进程。
    pub timeout_ms: u64,
    /// 最大内存（MB）。
    pub memory_limit_mb: u64,
}

impl PluginProcessSpec {
    pub fn validate(&self) -> crate::errors::AicoreResult<()> {
        use crate::errors::AicoreError;
        if self.plugin_id.trim().is_empty() {
            return Err(AicoreError::PluginInvalid("plugin_id is empty".to_string()));
        }
        if self.artifact_path.trim().is_empty() {
            return Err(AicoreError::PluginInvalid(
                "artifact_path is empty".to_string(),
            ));
        }
        if self.timeout_ms == 0 || self.timeout_ms > 300_000 {
            return Err(AicoreError::PluginInvalid(
                "timeout_ms must be in 1..=300000 ms".to_string(),
            ));
        }
        if self.memory_limit_mb == 0 {
            return Err(AicoreError::PluginInvalid(
                "memory_limit_mb must be > 0".to_string(),
            ));
        }
        Ok(())
    }
}

/// 插件进程启动结果。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginProcessHandle {
    pub handle_id: String,
    pub plugin_id: String,
    pub pid: u32,
    pub started_at_unix_ms: u64,
}

/// 插件崩溃事件。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginCrashEvent {
    pub plugin_id: String,
    pub handle_id: String,
    pub exit_code: Option<i32>,
    pub reason: String,
    pub detected_at_unix_ms: u64,
}

/// 插件 Host 契约（进程级隔离）。
///
/// 平台实现：
/// - Linux：`spawn` + `cgroups`（可选）
/// - Windows：`CreateProcess` + Job Object（可选）
pub trait IPluginHost: Send + Sync {
    /// 启动插件进程。
    fn spawn(&self, spec: &PluginProcessSpec) -> crate::errors::AicoreResult<PluginProcessHandle>;

    /// 停止插件进程（graceful；超时强杀）。
    fn stop(&self, handle: &PluginProcessHandle) -> crate::errors::AicoreResult<()>;

    /// 查询状态。
    fn state(&self, handle: &PluginProcessHandle) -> PluginHostState;

    /// 拉取最近崩溃事件。
    fn recent_crashes(&self) -> Vec<PluginCrashEvent>;

    /// 清空崩溃历史。
    fn clear_crashes(&self);

    fn host_name(&self) -> &'static str;
}
