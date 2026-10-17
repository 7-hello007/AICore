use serde::{Deserialize, Serialize};

/// Plugin process state.
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

/// Plugin process specifications.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginProcessSpec {
    pub plugin_id: String,
    pub artifact_path: String,
    /// Maximum execution time (ms). Post-time, host aborting process.
    pub timeout_ms: u64,
    /// OS-enforced memory budget in MiB: Windows job committed memory (whole
    /// process tree), Linux per-process virtual address space hard limit.
    /// This does not provide a filesystem/network sandbox for malicious code.
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

/// The result of the plugin process.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginProcessHandle {
    pub handle_id: String,
    pub plugin_id: String,
    pub pid: u32,
    pub started_at_unix_ms: u64,
}

/// Plugin crash event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginCrashEvent {
    pub plugin_id: String,
    pub handle_id: String,
    pub exit_code: Option<i32>,
    pub reason: String,
    pub detected_at_unix_ms: u64,
}

/// Plugin Host Compact (process level segregation).
///
/// Platform implementations provide:
/// - Linux: pre-exec hard address-space limit and owned process group.
/// - Windows: suspended creation, verified Job Object limits, then resume.
pub trait IPluginHost: Send + Sync {
    /// Starts the plugin process.
    fn spawn(&self, spec: &PluginProcessSpec) -> crate::errors::AicoreResult<PluginProcessHandle>;

    /// Stop the plug-in process (graceful; overtime killing).
    fn stop(&self, handle: &PluginProcessHandle) -> crate::errors::AicoreResult<()>;

    /// Query status.
    fn state(&self, handle: &PluginProcessHandle) -> PluginHostState;

    /// Pull the recent crash.
    fn recent_crashes(&self) -> Vec<PluginCrashEvent>;

    /// Clear the history of collapse.
    fn clear_crashes(&self);

    fn host_name(&self) -> &'static str;
}
