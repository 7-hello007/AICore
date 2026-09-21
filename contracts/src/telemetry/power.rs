use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PowerSnapshot {
    pub ac_connected: bool,
    pub power_plan: Option<String>,
    pub cpu_power_watts: Option<f32>,
    pub gpu_power_watts: Option<f32>,
}
