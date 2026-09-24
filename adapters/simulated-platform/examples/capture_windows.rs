//! Read-only Windows Adapter capture. It does not invoke any setter trait.
#[cfg(target_os = "windows")]
use aicore_adapter_simulated_platform::{PlatformFixture, ProcessState};
#[cfg(target_os = "windows")]
use aicore_adapter_windows::{
    WindowsCapabilityProvider, WindowsHardwareCapabilityProvider, WindowsHardwareTelemetryProvider,
    WindowsPowerController, WindowsProcessController, WindowsTelemetryProvider,
};
#[cfg(target_os = "windows")]
use aicore_contracts::{
    hardware::matrix::{DeviceCapabilityMatrix, HostOs},
    traits::{
        capability_provider::ICapabilityProvider, hardware_telemetry::IHardwareTelemetryProvider,
        power_controller::IPowerController, process_controller::IProcessController,
        telemetry_provider::ITelemetryProvider,
    },
};
#[cfg(target_os = "windows")]
use std::collections::BTreeMap;

#[cfg(target_os = "windows")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut snapshot = WindowsTelemetryProvider::new().collect()?;
    let capabilities = WindowsCapabilityProvider::new().capabilities_detailed();
    let process = WindowsProcessController::new();
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
    if processes.is_empty() {
        return Err("no captured process allowed read-only priority and affinity queries".into());
    }
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
    if snapshot.gpu.name.is_some() {
        snapshot.gpu.name = Some("<redacted>".into());
    }
    if snapshot.npu.name.is_some() {
        snapshot.npu.name = Some("<redacted>".into());
    }
    let power_profile = WindowsPowerController::new().get_profile().ok();
    let hardware = WindowsHardwareTelemetryProvider::new();
    let hardware_caps = WindowsHardwareCapabilityProvider::new();
    let mut gpu_samples: Vec<_> = hardware
        .gpu_devices()
        .iter()
        .filter_map(|d| hardware.sample_gpu(&d.id).ok())
        .collect();
    let mut npu_samples: Vec<_> = hardware
        .npu_devices()
        .iter()
        .filter_map(|d| hardware.sample_npu(&d.id).ok())
        .collect();
    let hardware_matrices = gpu_samples
        .iter()
        .filter_map(|sample| {
            DeviceCapabilityMatrix::from_discovered(
                HostOs::Windows,
                &sample.device,
                hardware_caps.gpu_capability(&sample.device.id),
            )
        })
        .chain(npu_samples.iter().filter_map(|sample| {
            DeviceCapabilityMatrix::from_discovered(
                HostOs::Windows,
                &sample.device,
                hardware_caps.npu_capability(&sample.device.id),
            )
        }))
        .collect();
    for sample in &mut gpu_samples {
        sample.device.name = "<redacted>".into();
        sample.device.driver = None;
    }
    for sample in &mut npu_samples {
        sample.device.name = "<redacted>".into();
        sample.device.driver = None;
    }
    let fixture = PlatformFixture {
        source: "WindowsTelemetryProvider + WindowsCapabilityProvider + WindowsHardwareTelemetryProvider + WindowsProcessController + WindowsPowerController; read-only; identifying strings redacted".into(),
        snapshot,
        capabilities,
        processes,
        power_profile,
        gpu_samples,
        npu_samples,
        hardware_matrices,
    };
    println!("{}", serde_json::to_string_pretty(&fixture)?);
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("read-only fixture capture is available on Windows only");
}
