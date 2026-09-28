use serde::{Deserialize, Serialize};

/// Plugin Declaration Permission.
///
/// Permissions are "capability requests," not "authorizations." Core check if user permission is allowed when loading.
///
/// serde indicates a dot format (consistent with `name()`), such as `"process.control"`.
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
    /// Whether or not it belongs to the "control class" (which requires explicit user authorization).
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
