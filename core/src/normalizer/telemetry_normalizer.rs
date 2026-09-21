use std::time::{SystemTime, UNIX_EPOCH};

use aicore_contracts::telemetry::ComputerSnapshot;

use crate::domain::computer_state::{ComputerState, SystemHealth};

/// 将原始 ComputerSnapshot 规范化为 ComputerState。
///
/// V2-A-1：
/// - 计算 `freshness_ms = now - collected_at_unix_ms`。
/// - 保留 V1 的百分比 clamp。
pub struct TelemetryNormalizer;

impl TelemetryNormalizer {
    pub fn new() -> Self {
        Self
    }

    pub fn normalize(&self, mut snapshot: ComputerSnapshot) -> ComputerState {
        clamp_percentages(&mut snapshot);
        snapshot.freshness_ms = compute_freshness_ms(snapshot.collected_at_unix_ms);

        let health = SystemHealth {
            cpu_available: snapshot.cpu.logical_cores > 0,
            gpu_available: snapshot.gpu.name.is_some(),
            npu_available: snapshot.npu.available,
            battery_present: snapshot.battery.present,
        };

        ComputerState { snapshot, health }
    }
}

impl Default for TelemetryNormalizer {
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

fn compute_freshness_ms(collected_at: Option<u64>) -> Option<u64> {
    collected_at.map(|t| now_unix_ms().saturating_sub(t))
}

fn clamp_percentages(snapshot: &mut ComputerSnapshot) {
    snapshot.cpu.usage_percent = snapshot.cpu.usage_percent.clamp(0.0, 100.0);
    snapshot.memory.usage_percent = snapshot.memory.usage_percent.clamp(0.0, 100.0);

    for core in &mut snapshot.cpu.per_core_usage {
        *core = core.clamp(0.0, 100.0);
    }
    if let Some(gpu) = snapshot.gpu.usage_percent.as_mut() {
        *gpu = gpu.clamp(0.0, 100.0);
    }
    for disk in &mut snapshot.disks {
        disk.usage_percent = disk.usage_percent.clamp(0.0, 100.0);
    }
}
