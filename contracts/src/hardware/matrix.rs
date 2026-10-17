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

impl HardwareVendor {
    /// PCI identity supplied by an OS/provider record, not display text.
    pub fn from_pci_vendor_id(vendor_id: u32) -> Self {
        match vendor_id {
            0x10de => Self::Nvidia,
            0x1002 => Self::Amd,
            0x8086 => Self::Intel,
            0x17cb => Self::Qualcomm,
            other => Self::Other(format!("PCI:{other:04x}")),
        }
    }
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
        // Display/manufacturer strings are descriptive, not typed authority.
        // Providers may set a typed vendor only from OS/provider numeric identity.
        let vendor = HardwareVendor::Other(device.vendor.clone());
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
        assert_eq!(matrix.vendor, HardwareVendor::Other("AMD".into()));
        assert!(!matrix.supports(HardwareCapability::VendorControl));
        assert_eq!(matrix.thermal_limit_celsius, None);
    }
}

#[cfg(test)]
mod p0_vendor_tests {
    use super::*;
    #[test]
    fn p0_display_vendor_is_not_typed_identity_authority() {
        let device = HardwareDeviceInfo {
            id: "spoof".into(),
            vendor: "NVIDIA".into(),
            name: "spoof".into(),
            driver: None,
        };
        let matrix = DeviceCapabilityMatrix::from_discovered(
            HostOs::Windows,
            &device,
            HardwareCapabilitySet::new("spoof"),
        )
        .unwrap();
        assert!(matches!(matrix.vendor, HardwareVendor::Other(_)));
    }
}
