pub mod capability;
pub mod matrix;
pub mod telemetry;

pub use capability::{HardwareCapability, HardwareCapabilityEntry, HardwareCapabilitySet};
pub use matrix::{DeviceCapabilityMatrix, HardwareVendor, HostOs};
pub use telemetry::{GpuTelemetrySample, HardwareDeviceInfo, NpuTelemetrySample};
