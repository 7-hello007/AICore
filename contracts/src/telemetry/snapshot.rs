use serde::{Deserialize, Serialize};

use super::{
    application::ApplicationSnapshot, battery::BatterySnapshot, cpu::CpuSnapshot,
    disk::DiskSnapshot, gpu::GpuSnapshot, memory::MemorySnapshot, network::NetworkSnapshot,
    npu::NpuSnapshot, power::PowerSnapshot, process::ProcessSnapshot,
    temperature::TemperatureSnapshot,
};

/// Unified cross-platform computer state snapshot.
///
/// V2-A-1：
/// - Replace `timestamp: SystemTime` with `collected_at_unix_ms` to facilitate the serialization of processes.
/// - Add `freshness_ms`: Calculated by Normalizer, indicating how long the data is from today.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ComputerSnapshot {
    /// Collect time, Unix epoch millisecond.
    pub collected_at_unix_ms: Option<u64>,

    /// Data freshness, milliseconds; 0 indicates that they have just been collected. Filled by Normalizer.
    pub freshness_ms: Option<u64>,

    pub cpu: CpuSnapshot,
    pub gpu: GpuSnapshot,
    pub npu: NpuSnapshot,
    pub memory: MemorySnapshot,
    pub disks: Vec<DiskSnapshot>,
    pub networks: Vec<NetworkSnapshot>,
    pub temperature: TemperatureSnapshot,
    pub battery: BatterySnapshot,
    pub power: PowerSnapshot,
    pub top_processes: Vec<ProcessSnapshot>,
    pub applications: Vec<ApplicationSnapshot>,
}
