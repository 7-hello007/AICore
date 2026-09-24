use serde::{Deserialize, Serialize};

use super::capability::{HardwareCapability, HardwareCapabilitySet};
use super::telemetry::HardwareDeviceInfo;

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

    /// Bind a capability declaration to the device discovered by the same
    /// adapter. A mismatched ID is discarded rather than advertised.
    pub fn from_discovered(
        os: HostOs,
        device: &HardwareDeviceInfo,
        capabilities: HardwareCapabilitySet,
    ) -> Option<Self> {
        if device.id != capabilities.device_name || device.id.is_empty() {
            return None;
        }
        let vendor = match device.vendor.to_ascii_lowercase().as_str() {
            "nvidia" => HardwareVendor::Nvidia,
            "amd" | "advanced micro devices" => HardwareVendor::Amd,
            "intel" => HardwareVendor::Intel,
            "qualcomm" => HardwareVendor::Qualcomm,
            _ => HardwareVendor::Other(device.vendor.clone()),
        };
        Some(Self {
            os,
            vendor,
            device_id: device.id.clone(),
            capabilities,
            thermal_limit_celsius: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovered_matrix_rejects_identity_mismatch_without_inventing_controls() {
        let device = HardwareDeviceInfo {
            id: "drm:card0".into(),
            vendor: "AMD".into(),
            name: "card0".into(),
            driver: None,
        };
        assert!(DeviceCapabilityMatrix::from_discovered(
            HostOs::Linux,
            &device,
            HardwareCapabilitySet::new("drm:card1")
        )
        .is_none());
        let matrix = DeviceCapabilityMatrix::from_discovered(
            HostOs::Linux,
            &device,
            HardwareCapabilitySet::new("drm:card0"),
        )
        .unwrap();
        assert_eq!(matrix.vendor, HardwareVendor::Amd);
        assert!(!matrix.supports(HardwareCapability::VendorControl));
        assert_eq!(matrix.thermal_limit_celsius, None);
    }
}
