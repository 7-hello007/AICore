use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct NpuSnapshot {
    pub name: Option<String>,
    pub usage_percent: Option<f32>,
    /// A valid utilization telemetry sample is available. Device detection,
    /// inference-provider availability and hardware control are separate facts.
    pub available: bool,
    pub vendor: Option<String>,
}
