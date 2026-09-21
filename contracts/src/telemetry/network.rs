use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct NetworkSnapshot {
    pub interface: String,
    pub received_bytes: u64,
    pub transmitted_bytes: u64,
    pub received_bytes_per_sec: u64,
    pub transmitted_bytes_per_sec: u64,
}
