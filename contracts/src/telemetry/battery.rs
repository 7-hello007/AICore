use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BatterySnapshot {
    pub present: bool,
    pub percentage: Option<f32>,
    pub charging: Option<bool>,
    pub time_remaining_sec: Option<u64>,
}
