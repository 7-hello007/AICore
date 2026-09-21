use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MemorySnapshot {
    pub total_mb: u64,
    pub used_mb: u64,
    pub available_mb: u64,
    pub usage_percent: f32,
    pub swap_total_mb: u64,
    pub swap_used_mb: u64,
}
