//! Compile portable Linux hardware modules against controlled files on any host.
#![allow(dead_code)]
#[path = "../../linux/src/hardware/executor.rs"]
mod executor;
#[path = "../../linux/src/gpu.rs"]
mod gpu;
#[path = "../../linux/src/telemetry/gpu.rs"]
mod gpu_telemetry;
#[path = "../../linux/src/telemetry/npu.rs"]
mod npu;
#[path = "../../linux/src/capability.rs"]
mod platform_capability;
#[path = "../../linux/src/telemetry/sysfs.rs"]
pub mod sysfs;

mod telemetry {
    pub use crate::sysfs;
}
mod hardware {
    pub mod telemetry {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../linux/src/hardware/telemetry.rs"
        ));
    }
    pub mod capability {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../linux/src/hardware/capability.rs"
        ));
    }
}
