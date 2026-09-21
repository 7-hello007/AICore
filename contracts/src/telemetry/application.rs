use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ApplicationSnapshot {
    pub name: String,
    pub pid: u32,
    pub is_foreground: bool,
    pub window_title: Option<String>,
}
