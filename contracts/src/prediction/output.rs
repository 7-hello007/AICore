use serde::{Deserialize, Serialize};

use crate::context::workload::WorkloadType;

/// Projections.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Prediction {
    /// Forecasting workload.
    pub predicted_workload: WorkloadType,
    /// Confidence, 0.0-1.0.
    pub confidence: f32,
    /// Forecast window, millisecond.
    pub horizon_ms: u64,
    /// Human readable explanation.
    pub reason: String,
    /// Projector name for audit.
    pub predictor_name: String,
}
