use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct NpuSnapshot {
    pub name: Option<String>,
    pub usage_percent: Option<f32>,
    pub available: bool,
    pub vendor: Option<String>,
}
