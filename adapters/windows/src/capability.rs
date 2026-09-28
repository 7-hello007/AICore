use std::process::Command;

use aicore_contracts::{
    capability::entry::CapabilityEntry, traits::capability_provider::ICapabilityProvider,
};

/// Windows capability provider.
///
/// Windows Process / Power use native OS APIs; vendor-specific hardware remains capability-gated.
pub struct WindowsCapabilityProvider;

impl WindowsCapabilityProvider {
    pub fn new() -> Self {
        Self
    }
    fn nvidia_available() -> bool {
        Command::new("nvidia-smi")
            .arg("-L")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

impl Default for WindowsCapabilityProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ICapabilityProvider for WindowsCapabilityProvider {
    fn capabilities_detailed(&self) -> Vec<CapabilityEntry> {
        vec![
            // Telemetry: always readable
            CapabilityEntry::supported("cpu.telemetry"),
            CapabilityEntry::supported("memory.telemetry"),
            CapabilityEntry::supported("disk.telemetry"),
            CapabilityEntry::supported("network.telemetry"),
            CapabilityEntry::supported("process.telemetry"),
            // Process / Power: provided via Windows API
            CapabilityEntry::supported("process.priority.read"),
            CapabilityEntry::conditional(
                "process.priority.write",
                "target PID access rights depend on the current Windows security token",
            ),
            CapabilityEntry::supported("power.profile.read"),
            CapabilityEntry::supported("power.profile.write"),
            // GPU, NPU, and temperature capabilities depend on the detected device and vendor runtime.
            if Self::nvidia_available() {
                CapabilityEntry::supported("gpu.telemetry")
            } else {
                CapabilityEntry::partial(
                    "gpu.telemetry",
                    "vendor/runtime dependent; NVIDIA supported when nvidia-smi is installed",
                )
            },
            if Self::nvidia_available() {
                CapabilityEntry::partial(
                    "gpu.control",
                    "NVIDIA power limit control available through nvidia-smi",
                )
            } else {
                CapabilityEntry::unsupported(
                    "gpu.control",
                    "vendor-specific GPU control requires vendor adapter/runtime",
                )
            },
            CapabilityEntry::unsupported(
                "npu.telemetry",
                "requires Windows ML/vendor runtime adapter",
            ),
            CapabilityEntry::unsupported(
                "npu.control",
                "requires vendor runtime; no generic cross-vendor control API",
            ),
            CapabilityEntry::unsupported(
                "temperature.telemetry",
                "best-effort through vendor/runtime sensors",
            ),
            CapabilityEntry::partial(
                "foreground.telemetry",
                "GetForegroundWindow wired; window title read best-effort",
            ),
            CapabilityEntry::supported("process.affinity.read"),
            CapabilityEntry::conditional(
                "process.affinity.write",
                "target PID access rights depend on the current Windows security token",
            ),
        ]
    }

    fn component_name(&self) -> &'static str {
        "windows_capability"
    }
}
