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

use super::{foreground::ForegroundCache, gpu, npu, power, temperature};

pub struct LinuxTelemetryProvider {
    system: Mutex<System>,
    disks: Mutex<Disks>,
    networks: Mutex<Networks>,
    last_network_sample: Mutex<Option<Instant>>,
    foreground: ForegroundCache,
}

impl LinuxTelemetryProvider {
    pub fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();

        Self {
            system: Mutex::new(system),
            disks: Mutex::new(Disks::new_with_refreshed_list()),
            networks: Mutex::new(Networks::new_with_refreshed_list()),
            last_network_sample: Mutex::new(None),
            foreground: ForegroundCache::new(500),
        }
    }
}

impl Default for LinuxTelemetryProvider {
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

impl ITelemetryProvider for LinuxTelemetryProvider {
    fn collect(&self) -> ContractResult<ComputerSnapshot> {
        // 1. Refresh sysinfo
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

        // 2. CPU
        let cpu = CpuSnapshot {
            usage_percent: system.global_cpu_info().cpu_usage(),
            temperature_celsius: None, // Uniquely filled by temperature module
            logical_cores: system.cpus().len() as u32,
            physical_cores: None,
            frequency_mhz: Some(system.global_cpu_info().frequency() as u32),
            per_core_usage: system.cpus().iter().map(|c| c.cpu_usage()).collect(),
        };

        // 3. Memory
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

        // 4. Disks
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

        // 5. Networks
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

        // Processes; attempting PID tag is_foreground
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

        // 7. Applications: a composite of a foreground window (if available)
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

        // 8. Temperature / GPU / NPU
        let temperature = temperature::read_temperature();
        let cpu_temp = temperature.cpu_celsius;
        let gpu_snap = gpu::read_gpu();
        let npu_snap = npu::read_npu();
        let (battery, power) = power::read_power();

        // CPU temperature backfill
        let mut cpu = cpu;
        cpu.temperature_celsius = cpu_temp;

        Ok(ComputerSnapshot {
            collected_at_unix_ms: Some(collected_at),
            freshness_ms: Some(0),
            cpu,
            gpu: gpu_snap,
            npu: npu_snap,
            memory,
            disks: disks_snap,
            networks: networks_snap,
            temperature,
            battery,
            power,
            top_processes: processes,
            applications,
        })
    }

    fn platform_name(&self) -> &'static str {
        "linux"
    }

    fn capabilities(&self) -> Vec<&'static str> {
        vec![
            "cpu.telemetry",
            "memory.telemetry",
            "disk.telemetry",
            "network.telemetry",
            "process.telemetry",
            "temperature.telemetry",
            "gpu.telemetry.best_effort",
            "npu.telemetry.best_effort",
            "foreground.telemetry.best_effort",
        ]
    }

    fn sampling_constraints(&self) -> SamplingConstraints {
        // 100ms is the minimum suggested interval to avoid over-costing the foreground window sub-process
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
