use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TemperatureSnapshot {
    pub cpu_celsius: Option<f32>,
    pub gpu_celsius: Option<f32>,
    pub system_celsius: Option<f32>,
}
