use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GpuSnapshot {
    pub name: Option<String>,
    pub usage_percent: Option<f32>,
    pub memory_used_mb: Option<u64>,
    pub memory_total_mb: Option<u64>,
    pub temperature_celsius: Option<f32>,
    pub vendor: Option<String>,
}
