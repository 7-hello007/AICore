use serde::{Deserialize, Serialize};

use super::capability::{HardwareCapability, HardwareCapabilitySet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostOs {
    Linux,
    Windows,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HardwareVendor {
    Nvidia,
    Amd,
    Intel,
    Qualcomm,
    Other(String),
}

/// Declaration, not proof: adapters must populate supported entries from
/// real platform discovery and tests must validate them on target hosts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceCapabilityMatrix {
    pub os: HostOs,
    pub vendor: HardwareVendor,
    pub device_id: String,
    pub capabilities: HardwareCapabilitySet,
    pub thermal_limit_celsius: Option<f32>,
}

impl DeviceCapabilityMatrix {
    pub fn supports(&self, capability: HardwareCapability) -> bool {
        self.capabilities.is_supported(capability)
    }
}
