use serde::{Deserialize, Serialize};

/// 插件声明的权限。
///
/// 权限是"能力请求"，不是"授权"。Core 在加载时校验用户是否允许。
///
/// serde 表示采用点分格式（与 `name()` 一致），例如 `"process.control"`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginPermission {
    #[serde(rename = "telemetry.read")]
    TelemetryRead,
    #[serde(rename = "process.read")]
    ProcessRead,
    #[serde(rename = "process.control")]
    ProcessControl,
    #[serde(rename = "gpu.read")]
    GpuRead,
    #[serde(rename = "npu.read")]
    NpuRead,
    #[serde(rename = "power.control")]
    PowerControl,
    #[serde(rename = "gpu.control")]
    GpuControl,
    #[serde(rename = "npu.control")]
    NpuControl,
    #[serde(rename = "memory.access")]
    MemoryAccess,
    #[serde(rename = "ai.provider")]
    AiProvider,
    #[serde(rename = "adapter.registration")]
    AdapterRegistration,
    #[serde(rename = "task.submit")]
    TaskSubmit,
    #[serde(rename = "plugin.install")]
    PluginInstall,
    #[serde(rename = "plugin.control")]
    PluginControl,
}

impl PluginPermission {
    /// 是否属于"控制类"权限（需要用户显式授权）。
    pub fn is_control(self) -> bool {
        matches!(
            self,
            Self::ProcessControl
                | Self::PowerControl
                | Self::GpuControl
                | Self::NpuControl
                | Self::AdapterRegistration
                | Self::TaskSubmit
                | Self::PluginInstall
                | Self::PluginControl
        )
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::TelemetryRead => "telemetry.read",
            Self::ProcessRead => "process.read",
            Self::ProcessControl => "process.control",
            Self::GpuRead => "gpu.read",
            Self::NpuRead => "npu.read",
            Self::PowerControl => "power.control",
            Self::GpuControl => "gpu.control",
            Self::NpuControl => "npu.control",
            Self::MemoryAccess => "memory.access",
            Self::AiProvider => "ai.provider",
            Self::AdapterRegistration => "adapter.registration",
            Self::TaskSubmit => "task.submit",
            Self::PluginInstall => "plugin.install",
            Self::PluginControl => "plugin.control",
        }
    }
}
