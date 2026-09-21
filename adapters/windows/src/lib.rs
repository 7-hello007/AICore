pub mod capability;
pub mod gpu;
pub mod hardware;
pub mod npu;
pub mod power;
pub mod process;
pub mod telemetry;

pub use capability::WindowsCapabilityProvider;
pub use gpu::WindowsGpuController;
pub use hardware::{
    WindowsHardwareCapabilityProvider, WindowsHardwareExecutor, WindowsHardwareTelemetryProvider,
};
pub use npu::WindowsNpuController;
pub use power::WindowsPowerController;
pub use process::WindowsProcessController;
pub use telemetry::WindowsTelemetryProvider;
