use serde::{Deserialize, Serialize};

use super::{
    application::ApplicationSnapshot, battery::BatterySnapshot, cpu::CpuSnapshot,
    disk::DiskSnapshot, gpu::GpuSnapshot, memory::MemorySnapshot, network::NetworkSnapshot,
    npu::NpuSnapshot, power::PowerSnapshot, process::ProcessSnapshot,
    temperature::TemperatureSnapshot,
};

/// 统一的跨平台计算机状态快照。
///
/// V2-A-1：
/// - 用 `collected_at_unix_ms` 替代 `timestamp: SystemTime`，便于跨进程序列化。
/// - 新增 `freshness_ms`：由 Normalizer 计算，表示数据距今多久。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ComputerSnapshot {
    /// 采集时刻，Unix epoch 毫秒。
    pub collected_at_unix_ms: Option<u64>,

    /// 数据新鲜度，毫秒；0 表示刚采集。由 Normalizer 填充。
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
