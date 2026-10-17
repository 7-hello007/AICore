use serde::{Deserialize, Serialize};

/// Hardware capability level.
///
/// For GPU / NPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HardwareCapability {
    /// Equipment identity (name, manufacturer, model).
    DeviceIdentity,
    /// Use rate telemetry.
    UtilizationTelemetry,
    /// Visibility / memory telemetry.
    MemoryTelemetry,
    /// Temperature telemetry.
    TemperatureTelemetry,
    /// Queue / session telemetry.
    QueueTelemetry,
    /// Workload hints (e.g. "graphics"/ "compute").
    WorkloadHint,
    /// Exchange Provider selection.
    ProviderSelection,
    /// Que / reserve.
    ResourceReservation,
    /// Model placement (e.g. "where to put NPU").
    ModelPlacement,
    /// Vendor-specific controls.
    VendorControl,
}

impl HardwareCapability {
    /// Whether to read telemetry only.
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

    /// Whether it is a control capability (which requires explicit authorization).
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

/// Status of individual hardware capabilities. `supported` describes the
/// provider's implemented/observed capability, never authorization, a current
/// availability guarantee, an applied operation, or independent verification.
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

/// Hardware device category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum HardwareDeviceKind {
    Gpu,
    Npu,
    #[default]
    Unknown,
}

/// List of hardware capabilities.
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
        Self {
            device_name,
            device_kind: HardwareDeviceKind::Unknown,
            entries: Vec::new(),
        }
    }

    /// Set by the discovering provider, never inferred from an untrusted ID.
    pub fn with_device_kind(mut self, kind: HardwareDeviceKind) -> Self {
        self.device_kind = kind;
        self
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

#[cfg(test)]
mod p0_identity_tests {
    use super::*;
    #[test]
    fn p0_id_prefix_does_not_authorize_a_device_kind() {
        for id in [
            "gpu:spoof",
            "nvidia:all",
            "drm:card0",
            "npu:spoof",
            "accel:0",
        ] {
            assert_eq!(
                HardwareCapabilitySet::new(id).device_kind,
                HardwareDeviceKind::Unknown
            );
        }
    }
}
