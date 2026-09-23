use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use aicore_contracts::{
    errors::{ContractError, ContractResult},
    telemetry::{
        constraints::SamplingConstraints, cpu::CpuSnapshot, disk::DiskSnapshot,
        memory::MemorySnapshot, network::NetworkSnapshot, process::ProcessSnapshot,
        snapshot::ComputerSnapshot,
    },
    traits::telemetry_provider::ITelemetryProvider,
};
use sysinfo::{Disks, Networks, System};

use super::foreground::ForegroundCache;
use crate::hardware::telemetry::WindowsHardwareTelemetryProvider;
use aicore_contracts::traits::hardware_telemetry::IHardwareTelemetryProvider;

/// Windows 遥测 Provider。
///
/// V2-A-1：与 Linux 对称，内部 `Mutex` 刷新。
pub struct WindowsTelemetryProvider {
    system: Mutex<System>,
    disks: Mutex<Disks>,
    networks: Mutex<Networks>,
    last_network_sample: Mutex<Option<Instant>>,
    foreground: ForegroundCache,
    hardware: WindowsHardwareTelemetryProvider,
}

impl WindowsTelemetryProvider {
    pub fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();

        Self {
            system: Mutex::new(system),
            disks: Mutex::new(Disks::new_with_refreshed_list()),
            networks: Mutex::new(Networks::new_with_refreshed_list()),
            last_network_sample: Mutex::new(None),
            foreground: ForegroundCache::new(500),
            hardware: WindowsHardwareTelemetryProvider::new(),
        }
    }
}

impl Default for WindowsTelemetryProvider {
    fn default() -> Self {
        Self::new()
    }
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn lock_err(what: &str) -> ContractError {
    ContractError::AdapterUnavailable(format!("{} lock poisoned", what))
}

impl ITelemetryProvider for WindowsTelemetryProvider {
    fn collect(&self) -> ContractResult<ComputerSnapshot> {
        let mut system = self.system.lock().map_err(|_| lock_err("system"))?;
        system.refresh_cpu();
        system.refresh_memory();
        system.refresh_processes();

        let mut disks = self.disks.lock().map_err(|_| lock_err("disks"))?;
        disks.refresh();

        let mut networks = self.networks.lock().map_err(|_| lock_err("networks"))?;
        networks.refresh();
        let mut last_network_sample = self
            .last_network_sample
            .lock()
            .map_err(|_| lock_err("network sample"))?;
        let now = Instant::now();
        let elapsed_ms = last_network_sample
            .replace(now)
            .map(|previous| now.duration_since(previous).as_millis() as u64)
            .unwrap_or(0);

        let collected_at = now_unix_ms();

        let cpu = CpuSnapshot {
            usage_percent: system.global_cpu_info().cpu_usage(),
            temperature_celsius: None,
            logical_cores: system.cpus().len() as u32,
            physical_cores: None,
            frequency_mhz: Some(system.global_cpu_info().frequency() as u32),
            per_core_usage: system.cpus().iter().map(|c| c.cpu_usage()).collect(),
        };

        let total_mb = system.total_memory() / 1024 / 1024;
        let used_mb = system.used_memory() / 1024 / 1024;
        let available_mb = total_mb.saturating_sub(used_mb);
        let usage_percent = if total_mb > 0 {
            (used_mb as f32 / total_mb as f32) * 100.0
        } else {
            0.0
        };
        let memory = MemorySnapshot {
            total_mb,
            used_mb,
            available_mb,
            usage_percent,
            swap_total_mb: system.total_swap() / 1024 / 1024,
            swap_used_mb: system.used_swap() / 1024 / 1024,
        };

        let disks_snap = disks
            .iter()
            .map(|d| {
                let total = d.total_space() as f64 / 1024.0 / 1024.0 / 1024.0;
                let available = d.available_space() as f64 / 1024.0 / 1024.0 / 1024.0;
                let used = (total - available).max(0.0);
                let usage_percent = if total > 0.0 {
                    (used / total * 100.0) as f32
                } else {
                    0.0
                };
                DiskSnapshot {
                    mount_point: d.mount_point().to_string_lossy().to_string(),
                    total_gb: total,
                    used_gb: used,
                    available_gb: available,
                    usage_percent,
                }
            })
            .collect();

        let networks_snap = networks
            .iter()
            .map(|(name, data)| NetworkSnapshot {
                interface: name.clone(),
                received_bytes: data.total_received(),
                transmitted_bytes: data.total_transmitted(),
                received_bytes_per_sec: rate_per_second(data.received(), elapsed_ms),
                transmitted_bytes_per_sec: rate_per_second(data.transmitted(), elapsed_ms),
            })
            .collect();

        let fg = self.foreground.read();
        let fg_pid = fg.pid;

        let mut processes: Vec<ProcessSnapshot> = system
            .processes()
            .iter()
            .map(|(pid, p)| {
                let pid_u32 = pid.as_u32();
                ProcessSnapshot {
                    pid: pid_u32,
                    name: p.name().to_string(),
                    cpu_usage_percent: p.cpu_usage(),
                    memory_mb: p.memory() / 1024 / 1024,
                    is_foreground: Some(pid_u32) == fg_pid,
                }
            })
            .collect();
        processes.sort_by(|a, b| {
            b.cpu_usage_percent
                .partial_cmp(&a.cpu_usage_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        processes.truncate(20);

        let applications = if let Some(pid) = fg_pid {
            let name = system
                .processes()
                .get(&sysinfo::Pid::from_u32(pid))
                .map(|p| p.name().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            vec![
                aicore_contracts::telemetry::application::ApplicationSnapshot {
                    name,
                    pid,
                    is_foreground: true,
                    window_title: fg.window_title.clone(),
                },
            ]
        } else {
            Vec::new()
        };

        let gpu = self
            .hardware
            .gpu_devices()
            .into_iter()
            .next()
            .and_then(|d| self.hardware.sample_gpu(&d.id).ok())
            .map(|g| aicore_contracts::telemetry::gpu::GpuSnapshot {
                name: Some(g.device.name),
                usage_percent: g.utilization_percent,
                memory_used_mb: g.memory_used_mb,
                memory_total_mb: g.memory_total_mb,
                temperature_celsius: g.temperature_celsius,
                vendor: Some(g.device.vendor),
            })
            .unwrap_or_default();
        let npu = self
            .hardware
            .npu_devices()
            .into_iter()
            .next()
            .map(|d| aicore_contracts::telemetry::npu::NpuSnapshot {
                name: Some(d.name),
                available: true,
                vendor: Some(d.vendor),
                usage_percent: self
                    .hardware
                    .sample_npu(&d.id)
                    .ok()
                    .and_then(|s| s.utilization_percent),
            })
            .unwrap_or_default();

        Ok(ComputerSnapshot {
            collected_at_unix_ms: Some(collected_at),
            freshness_ms: Some(0),
            cpu,
            gpu,
            npu,
            memory,
            disks: disks_snap,
            networks: networks_snap,
            top_processes: processes,
            applications,
            ..Default::default()
        })
    }

    fn platform_name(&self) -> &'static str {
        "windows"
    }

    fn capabilities(&self) -> Vec<&'static str> {
        vec![
            "cpu.telemetry",
            "memory.telemetry",
            "disk.telemetry",
            "network.telemetry",
            "process.telemetry",
            "foreground.telemetry.best_effort",
        ]
    }

    fn sampling_constraints(&self) -> SamplingConstraints {
        SamplingConstraints::new(100)
    }
}

fn rate_per_second(bytes_since_refresh: u64, elapsed_ms: u64) -> u64 {
    if elapsed_ms == 0 {
        0
    } else {
        bytes_since_refresh.saturating_mul(1_000) / elapsed_ms
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn network_rate_uses_refresh_delta_and_elapsed_time() {
        assert_eq!(super::rate_per_second(500, 250), 2_000);
        assert_eq!(super::rate_per_second(500, 0), 0);
    }
}
