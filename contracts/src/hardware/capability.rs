use serde::{Deserialize, Serialize};

/// 硬件能力等级。
///
/// 用于 GPU / NPU 的**细粒度**能力声明。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HardwareCapability {
    /// 设备身份（名称、厂商、型号）。
    DeviceIdentity,
    /// 使用率遥测。
    UtilizationTelemetry,
    /// 显存 / 内存遥测。
    MemoryTelemetry,
    /// 温度遥测。
    TemperatureTelemetry,
    /// 队列 / 会话遥测。
    QueueTelemetry,
    /// Workload 提示（如 "graphics" / "compute"）。
    WorkloadHint,
    /// Execution Provider 选择。
    ProviderSelection,
    /// 资源预留（queue / reservation）。
    ResourceReservation,
    /// 模型放置（如"放到哪块 NPU"）。
    ModelPlacement,
    /// 厂商专用控制。
    VendorControl,
}

impl HardwareCapability {
    /// 是否为只读遥测能力。
    pub fn is_telemetry(self) -> bool {
        matches!(
            self,
            Self::DeviceIdentity
                | Self::UtilizationTelemetry
                | Self::MemoryTelemetry
                | Self::TemperatureTelemetry
                | Self::QueueTelemetry
        )
    }

    /// 是否为控制类能力（需要显式授权）。
    pub fn is_control(self) -> bool {
        !self.is_telemetry()
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::DeviceIdentity => "device.identity",
            Self::UtilizationTelemetry => "device.utilization",
            Self::MemoryTelemetry => "device.memory",
            Self::TemperatureTelemetry => "device.temperature",
            Self::QueueTelemetry => "device.queue",
            Self::WorkloadHint => "control.workload_hint",
            Self::ProviderSelection => "control.provider_selection",
            Self::ResourceReservation => "control.resource_reservation",
            Self::ModelPlacement => "control.model_placement",
            Self::VendorControl => "control.vendor_specific",
        }
    }
}

/// 单个硬件能力的状态。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HardwareCapabilityEntry {
    pub capability: HardwareCapability,
    pub supported: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl HardwareCapabilityEntry {
    pub fn supported(capability: HardwareCapability) -> Self {
        Self {
            capability,
            supported: true,
            detail: None,
        }
    }

    pub fn unsupported(capability: HardwareCapability, detail: impl Into<String>) -> Self {
        Self {
            capability,
            supported: false,
            detail: Some(detail.into()),
        }
    }
}

/// 硬件设备类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum HardwareDeviceKind {
    Gpu,
    Npu,
    #[default]
    Unknown,
}

/// 硬件能力清单。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HardwareCapabilitySet {
    pub device_name: String,
    #[serde(default)]
    pub device_kind: HardwareDeviceKind,
    pub entries: Vec<HardwareCapabilityEntry>,
}

impl HardwareCapabilitySet {
    pub fn new(device_name: impl Into<String>) -> Self {
        let device_name = device_name.into();
        let device_kind = if device_name.starts_with("drm:")
            || device_name.starts_with("nvidia:")
            || device_name.starts_with("gpu:")
        {
            HardwareDeviceKind::Gpu
        } else if device_name.starts_with("npu:") || device_name.starts_with("accel:") {
            HardwareDeviceKind::Npu
        } else {
            HardwareDeviceKind::Unknown
        };
        Self {
            device_name,
            device_kind,
            entries: Vec::new(),
        }
    }

    pub fn with(mut self, entry: HardwareCapabilityEntry) -> Self {
        self.entries.push(entry);
        self
    }

    pub fn is_supported(&self, cap: HardwareCapability) -> bool {
        self.entries
            .iter()
            .any(|e| e.capability == cap && e.supported)
    }

    pub fn supported_controls(&self) -> Vec<HardwareCapability> {
        self.entries
            .iter()
            .filter(|e| e.supported && e.capability.is_control())
            .map(|e| e.capability)
            .collect()
    }

    pub fn supported_telemetry(&self) -> Vec<HardwareCapability> {
        self.entries
            .iter()
            .filter(|e| e.supported && e.capability.is_telemetry())
            .map(|e| e.capability)
            .collect()
    }
}
