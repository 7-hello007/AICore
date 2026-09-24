//! Read-only Linux capture for a simulator fixture. Never calls setter traits.
#[cfg(target_os = "linux")]
use aicore_adapter_linux::{
    LinuxCapabilityProvider, LinuxHardwareCapabilityProvider, LinuxHardwareTelemetryProvider,
    LinuxPowerController, LinuxProcessController, LinuxTelemetryProvider,
};
#[cfg(target_os = "linux")]
use aicore_adapter_simulated_platform::{PlatformFixture, ProcessState};
#[cfg(target_os = "linux")]
use aicore_contracts::{
    hardware::matrix::{DeviceCapabilityMatrix, HostOs},
    traits::{
        capability_provider::ICapabilityProvider, hardware_telemetry::IHardwareTelemetryProvider,
        power_controller::IPowerController, process_controller::IProcessController,
        telemetry_provider::ITelemetryProvider,
    },
};
#[cfg(target_os = "linux")]
use std::collections::BTreeMap;

#[cfg(target_os = "linux")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let telemetry = LinuxTelemetryProvider::new();
    let mut snapshot = telemetry.collect()?;
    let capabilities = LinuxCapabilityProvider::new().capabilities_detailed();
    let process = LinuxProcessController::new();
    let mut processes = BTreeMap::new();
    for sample in &snapshot.top_processes {
        if let (Ok(priority), Ok(affinity)) = (
            process.get_priority(sample.pid),
            process.get_affinity(sample.pid),
        ) {
            processes.insert(
                sample.pid,
                ProcessState {
                    priority,
                    affinity_cores: affinity.cores,
                },
            );
        }
    }
    // Preserve measured numeric values while removing identifying text.
    for sample in &mut snapshot.top_processes {
        sample.name = "<redacted>".into();
    }
    snapshot.applications.clear();
    for disk in &mut snapshot.disks {
        disk.mount_point = "<redacted>".into();
    }
    for network in &mut snapshot.networks {
        network.interface = "<redacted>".into();
    }
    let power_profile = LinuxPowerController::new().get_profile().ok();
    let hardware = LinuxHardwareTelemetryProvider::new();
    let hardware_caps = LinuxHardwareCapabilityProvider::new();
    let gpu_devices = hardware.gpu_devices();
    let npu_devices = hardware.npu_devices();
    let gpu_samples: Vec<_> = gpu_devices
        .iter()
        .filter_map(|d| hardware.sample_gpu(&d.id).ok())
        .collect();
    let npu_samples: Vec<_> = npu_devices
        .iter()
        .filter_map(|d| hardware.sample_npu(&d.id).ok())
        .collect();
    let hardware_matrices = gpu_samples
        .iter()
        .filter_map(|sample| {
            let d = &sample.device;
            DeviceCapabilityMatrix::from_discovered(
                HostOs::Linux,
                d,
                hardware_caps.gpu_capability(&d.id),
            )
        })
        .chain(npu_samples.iter().filter_map(|sample| {
            let d = &sample.device;
            DeviceCapabilityMatrix::from_discovered(
                HostOs::Linux,
                d,
                hardware_caps.npu_capability(&d.id),
            )
        }))
        .collect();
    let fixture = PlatformFixture {
        source: "LinuxTelemetryProvider + LinuxCapabilityProvider + LinuxHardwareTelemetryProvider + LinuxProcessController + LinuxPowerController; read-only; identifying strings redacted".into(),
        snapshot, capabilities, processes, power_profile,
        gpu_samples, npu_samples, hardware_matrices,
    };
    println!("{}", serde_json::to_string_pretty(&fixture)?);
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("read-only fixture capture is available on Linux only");
}
