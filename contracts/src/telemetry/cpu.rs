use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CpuSnapshot {
    pub usage_percent: f32,
    pub temperature_celsius: Option<f32>,
    pub logical_cores: u32,
    pub physical_cores: Option<u32>,
    pub frequency_mhz: Option<u32>,
    pub per_core_usage: Vec<f32>,
}
