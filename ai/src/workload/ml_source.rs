use std::sync::Arc;

use aicore_contracts::{
    context::workload::WorkloadType,
    errors::{ContractError, ContractResult},
    telemetry::snapshot::ComputerSnapshot,
    traits::workload_source::{IWorkloadSource, SourceKind, WorkloadObservation},
};

/// 特征向量：`[cpu_norm, gpu_norm, npu_norm, mem_norm]`。
pub const FEATURE_DIM: usize = 4;

/// 模型权重表。
#[derive(Debug, Clone)]
pub struct MlWeightTable {
    pub version: String,
    pub entries: Vec<MlWeightEntry>,
}

#[derive(Debug, Clone)]
pub struct MlWeightEntry {
    pub workload: WorkloadType,
    pub weights: [f32; FEATURE_DIM],
}

impl MlWeightTable {
    pub fn default_weights() -> Self {
        // 设计原则：
        // - 高资源 workload（Gaming / Rendering / VideoEditing）用**负的 GPU 权重**
        //   表达"没有 GPU 使用就不像这类"。
        // - 低资源 workload（GeneralOffice / BackgroundTask）权重与资源弱相关，
        //   使"全低"输入时能胜出。
        // - AiInference 强依赖 NPU；Compilation 强依赖 CPU。
        Self {
            version: "1.0.0".to_string(),
            entries: vec![
                MlWeightEntry {
                    workload: WorkloadType::Gaming,
                    weights: [0.6, 1.2, -0.3, 0.2],
                },
                MlWeightEntry {
                    workload: WorkloadType::Rendering,
                    weights: [0.7, 1.1, -0.3, 0.3],
                },
                MlWeightEntry {
                    workload: WorkloadType::AiInference,
                    weights: [0.3, 0.4, 1.2, 0.3],
                },
                MlWeightEntry {
                    workload: WorkloadType::Compilation,
                    weights: [1.0, -0.1, -0.3, 0.3],
                },
                MlWeightEntry {
                    workload: WorkloadType::VideoEditing,
                    weights: [0.5, 0.9, -0.3, 0.5],
                },
                MlWeightEntry {
                    workload: WorkloadType::GeneralOffice,
                    weights: [0.5, 0.3, 0.1, 0.5],
                },
                MlWeightEntry {
                    workload: WorkloadType::BackgroundTask,
                    weights: [0.7, 0.4, 0.1, 0.7],
                },
            ],
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version.trim().is_empty() {
            return Err("version is empty".to_string());
        }
        if self.entries.is_empty() {
            return Err("no entries".to_string());
        }
        Ok(())
    }
}

impl Default for MlWeightTable {
    fn default() -> Self {
        Self::default_weights()
    }
}

pub struct MlWorkloadSource {
    weights: Arc<MlWeightTable>,
}

impl MlWorkloadSource {
    pub fn new(weights: MlWeightTable) -> ContractResult<Self> {
        weights
            .validate()
            .map_err(|e| ContractError::InvalidRequest(format!("invalid weights: {}", e)))?;
        Ok(Self {
            weights: Arc::new(weights),
        })
    }

    pub fn with_default_weights() -> ContractResult<Self> {
        Self::new(MlWeightTable::default_weights())
    }

    pub fn weights(&self) -> &MlWeightTable {
        &self.weights
    }

    fn features(snapshot: &ComputerSnapshot) -> [f32; FEATURE_DIM] {
        [
            (snapshot.cpu.usage_percent / 100.0).clamp(0.0, 1.0),
            (snapshot.gpu.usage_percent.unwrap_or(0.0) / 100.0).clamp(0.0, 1.0),
            (snapshot.npu.usage_percent.unwrap_or(0.0) / 100.0).clamp(0.0, 1.0),
            (snapshot.memory.usage_percent / 100.0).clamp(0.0, 1.0),
        ]
    }
}

impl IWorkloadSource for MlWorkloadSource {
    fn observe(&self, snapshot: &ComputerSnapshot) -> ContractResult<WorkloadObservation> {
        let features = Self::features(snapshot);

        let scores: Vec<(WorkloadType, f32)> = self
            .weights
            .entries
            .iter()
            .map(|e| {
                let s: f32 = features
                    .iter()
                    .zip(e.weights.iter())
                    .map(|(a, b)| a * b)
                    .sum();
                (e.workload, s)
            })
            .collect();

        let max_score = scores
            .iter()
            .map(|(_, s)| *s)
            .fold(f32::NEG_INFINITY, f32::max);
        let exps: Vec<f32> = scores.iter().map(|(_, s)| (s - max_score).exp()).collect();
        let sum: f32 = exps.iter().sum();
        let probs: Vec<f32> = exps.iter().map(|e| e / sum).collect();

        let (best_idx, &best_prob) = probs
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .ok_or_else(|| ContractError::AiInferenceFailed("softmax failed".to_string()))?;

        let workload = scores[best_idx].0;

        let evidence = vec![
            format!(
                "ml_features=[{:.2},{:.2},{:.2},{:.2}]",
                features[0], features[1], features[2], features[3]
            ),
            format!("ml_prediction={:?} prob={:.3}", workload, best_prob),
        ];

        Ok(WorkloadObservation {
            workload_type: workload,
            confidence: best_prob,
            evidence,
        })
    }

    fn source_name(&self) -> &'static str {
        "ml.workload_source"
    }

    fn priority(&self) -> u32 {
        50
    }

    fn kind(&self) -> SourceKind {
        SourceKind::Ml
    }

    fn confidence_ceiling(&self) -> f32 {
        0.85
    }
}
