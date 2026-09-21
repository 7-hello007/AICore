use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ProcessSnapshot {
    pub pid: u32,
    pub name: String,
    pub cpu_usage_percent: f32,
    pub memory_mb: u64,
    pub is_foreground: bool,
}
