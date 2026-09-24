use aicore_contracts::policy::priority::PolicyPriority;
use aicore_contracts::telemetry::snapshot::ComputerSnapshot;
use serde::{Deserialize, Serialize};

/// 插件进程 IPC 请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HostRequest {
    /// Ping 探活。
    Ping,
    /// 执行插件逻辑。
    Execute { input: u32 },
    /// Typed, read-only telemetry snapshot capability.
    ReadTelemetry,
    /// Typed process priority operation; activation checks the exact pin.
    SetPriority { pid: u32, priority: PolicyPriority },
    /// 请求关闭。
    Shutdown,
}

/// 插件进程 IPC 响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HostResponse {
    Pong,
    Result { output: u32 },
    Telemetry { snapshot: Box<ComputerSnapshot> },
    PrioritySet { pid: u32, priority: PolicyPriority },
    ShuttingDown,
    Error { message: String },
}

pub const MAX_LINE_BYTES: usize = 64 * 1024;
