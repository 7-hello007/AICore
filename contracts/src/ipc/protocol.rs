use serde::{Deserialize, Serialize};

use crate::application::id::ApplicationId;
use crate::capability::entry::CapabilityEntry;
use crate::context::snapshot::ContextSnapshot;
use crate::policy::resource_policy::ResourcePolicy;
use crate::runtime::daemon_status::DaemonStatus;
use crate::schema::SchemaVersion;
use crate::task::contract::TaskContract;

/// IPC 协议版本。不兼容变更需递增 major。
pub const IPC_PROTOCOL_VERSION: SchemaVersion = SchemaVersion::CURRENT;

/// 请求 / 响应 Envelope。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Envelope {
    pub version: SchemaVersion,
    pub request_id: String,
    pub timestamp_unix_ms: u64,
    pub payload: Request,
}

impl Envelope {
    pub fn new(request_id: impl Into<String>, payload: Request) -> Self {
        Self {
            version: IPC_PROTOCOL_VERSION,
            request_id: request_id.into(),
            timestamp_unix_ms: now_unix_ms(),
            payload,
        }
    }
}

/// 客户端请求。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Request {
    /// 注册应用。
    Register {
        application_id: ApplicationId,
        name: String,
        version: String,
        capabilities: Vec<String>,
    },
    /// 注销应用。
    Unregister { application_id: ApplicationId },
    /// 提交 Task Contract。
    SubmitTask { contract: TaskContract },
    /// 查询 daemon 状态。
    GetStatus,
    /// 查询能力列表。
    GetCapabilities,
    /// 查询最近一次上下文。
    GetContext,
    /// 查询当前策略。
    GetPolicy,
    /// 查询最近 N 条 tick 摘要。
    GetRecentTicks { limit: usize },
    /// 查询所有插件。
    ListPlugins,
    /// V3.2：一次拉取完整 Dashboard 快照。
    GetDashboardSnapshot,
}

/// 响应 Envelope。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResponseEnvelope {
    pub version: SchemaVersion,
    pub request_id: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<ResponseData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<IpcError>,
}

impl ResponseEnvelope {
    pub fn ok(request_id: impl Into<String>, data: ResponseData) -> Self {
        Self {
            version: IPC_PROTOCOL_VERSION,
            request_id: request_id.into(),
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(request_id: impl Into<String>, error: IpcError) -> Self {
        Self {
            version: IPC_PROTOCOL_VERSION,
            request_id: request_id.into(),
            success: false,
            data: None,
            error: Some(error),
        }
    }
}

/// 响应数据。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
// ResponseData 变体大小差异较大（Status 含完整 DaemonStatus，
// DashboardSnapshot 已 box）。IPC 传输按引用处理，不做高频 clone，
// 因此不把 Status 也 box 掉。若后续按值分发，需重新评估。
#[allow(clippy::large_enum_variant)]
pub enum ResponseData {
    Registered {
        application_id: String,
    },
    Unregistered {
        removed: bool,
    },
    TaskSubmitted {
        accepted: bool,
        task_id: Option<String>,
        message: String,
    },
    Status {
        status: DaemonStatus,
    },
    Capabilities {
        component: String,
        entries: Vec<CapabilityEntry>,
    },
    Context {
        context: Option<ContextSnapshot>,
    },
    Policy {
        policy: Option<ResourcePolicy>,
    },
    RecentTicks {
        ticks: Vec<crate::runtime::daemon_status::TickSummary>,
    },
    Plugins {
        entries: Vec<PluginEntryDto>,
    },
    DashboardSnapshot {
        snapshot: Box<crate::dashboard::snapshot::DashboardSnapshot>,
    },
}

/// 插件条目 DTO（IPC 传输用，避免直接暴露 registry 内部类型）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginEntryDto {
    pub name: String,
    pub version: String,
    pub api_version: String,
    pub kind: String,
    pub trust_level: String,
    pub state: String,
    pub interfaces: Vec<String>,
    pub permissions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_reason: Option<String>,
    #[serde(default)]
    pub failure_count: u32,
}

/// IPC 错误。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IpcError {
    pub code: String,
    pub message: String,
}

impl IpcError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn invalid_request(msg: impl Into<String>) -> Self {
        Self::new("ERR_INVALID_REQUEST", msg)
    }

    pub fn not_supported(msg: impl Into<String>) -> Self {
        Self::new("ERR_NOT_SUPPORTED", msg)
    }

    pub fn protocol_mismatch(got: SchemaVersion, expected: SchemaVersion) -> Self {
        Self::new(
            "ERR_PROTOCOL_MISMATCH",
            format!(
                "protocol version mismatch: got {}, expected {}",
                got, expected
            ),
        )
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::new("ERR_INTERNAL", msg)
    }
}

fn now_unix_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
