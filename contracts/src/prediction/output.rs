use serde::{Deserialize, Serialize};

use crate::context::workload::WorkloadType;

/// 预测结果。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Prediction {
    /// 预测的 workload。
    pub predicted_workload: WorkloadType,
    /// 置信度，0.0 - 1.0。
    pub confidence: f32,
    /// 预测窗口，毫秒。
    pub horizon_ms: u64,
    /// 人类可读的解释。
    pub reason: String,
    /// 预测器名称，用于审计。
    pub predictor_name: String,
}
