use serde::{Deserialize, Serialize};

/// Service status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceState {
    NotInstalled,
    Stopped,
    Starting,
    Running,
    Stopping,
    Failed,
    Unknown,
}

impl ServiceState {
    pub fn is_operational(self) -> bool {
        matches!(self, Self::Running)
    }
}

/// Service configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub binary_path: String,
    pub args: Vec<String>,
    pub auto_start: bool,
    /// Failed to restart automatically.
    pub restart_on_failure: bool,
    /// Restart delay, seconds.
    pub restart_delay_secs: u64,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            name: "aicored".to_string(),
            display_name: "AICore Daemon".to_string(),
            description: "AI Computer Control Layer".to_string(),
            binary_path: String::new(),
            args: vec!["--interval-ms".to_string(), "100".to_string()],
            auto_start: true,
            restart_on_failure: true,
            restart_delay_secs: 5,
        }
    }
}

impl ServiceConfig {
    pub fn validate(&self) -> crate::errors::AicoreResult<()> {
        use crate::errors::AicoreError;
        if self.name.trim().is_empty() {
            return Err(AicoreError::ConfigInvalid(
                "service name is empty".to_string(),
            ));
        }
        if self.binary_path.trim().is_empty() {
            return Err(AicoreError::ConfigInvalid(
                "binary_path is empty".to_string(),
            ));
        }
        Ok(())
    }
}

/// Service results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServiceOperationResult {
    pub state: ServiceState,
    pub success: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl ServiceOperationResult {
    pub fn ok(state: ServiceState) -> Self {
        Self {
            state,
            success: true,
            message: None,
        }
    }

    pub fn failed(state: ServiceState, message: impl Into<String>) -> Self {
        Self {
            state,
            success: false,
            message: Some(message.into()),
        }
    }
}

/// Service life cycle compact.
///
/// Platform implementations provide:
/// Windows: `aicore-service-host` provides `sc.exe` / `windows-service` crate packaging.
/// - Linux：systemd unit。
pub trait IServiceController: Send + Sync {
    fn install(&self, config: &ServiceConfig) -> ServiceOperationResult;
    fn uninstall(&self) -> ServiceOperationResult;
    fn start(&self) -> ServiceOperationResult;
    fn stop(&self) -> ServiceOperationResult;
    fn upgrade(&self, new_binary_path: &str) -> ServiceOperationResult;
    fn rollback(&self) -> ServiceOperationResult;
    fn query_state(&self) -> ServiceState;
    fn controller_name(&self) -> &'static str;
}
