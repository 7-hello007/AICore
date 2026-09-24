use aicore_contracts::{
    context::workload::WorkloadType,
    errors::{ContractError, ContractResult},
    prediction::{input::PredictionInput, output::Prediction},
    traits::predictor::IPredictor,
};

/// 短期预测器。
///
/// 策略（按优先级）：
/// 1. **一致性优先**：若 Memory 中最近 `min_samples` 条记录都是同一 workload，
///    预测下一个还是它。
/// 2. **时间模式**：相同 UTC 小时有足量且高置信的个人样本时使用它。
/// 3. **多数优先**：若 Memory 样本数够，用 `dominant_workload`。
/// 4. 都不可用时返回 Err，让上层回退到当前观测。
///
/// 置信度：
/// - 一致性场景：样本数越多越自信，上限 0.95。
/// - 多数场景：固定 0.5。
pub struct ShortHorizonPredictor {
    min_samples: usize,
    horizon_ms: u64,
    min_confidence: f32,
}

impl ShortHorizonPredictor {
    pub fn new(min_samples: usize, horizon_ms: u64, min_confidence: f32) -> Self {
        Self {
            min_samples: min_samples.max(1),
            horizon_ms,
            min_confidence: min_confidence.clamp(0.0, 1.0),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(3, 5_000, 0.5)
    }

    pub fn min_samples(&self) -> usize {
        self.min_samples
    }

    pub fn min_confidence(&self) -> f32 {
        self.min_confidence
    }

    fn try_consistent(&self, recent: &[WorkloadType], sample_count: usize) -> Option<Prediction> {
        if recent.len() < self.min_samples {
            return None;
        }
        let first = recent[0];
        if first == WorkloadType::Unknown {
            return None;
        }
        let consistent = recent.iter().take(self.min_samples).all(|w| *w == first);
        if !consistent {
            return None;
        }

        // 样本越多越自信：5 条 → 0.6，10 条 → 0.8，20+ 条 → 0.95
        let confidence = (0.4 + (sample_count as f32) * 0.04).clamp(0.5, 0.95);
        if confidence < self.min_confidence {
            return None;
        }

        Some(Prediction {
            predicted_workload: first,
            confidence,
            horizon_ms: self.horizon_ms,
            reason: format!(
                "last {} workloads consistent: {:?} (samples={})",
                self.min_samples, first, sample_count
            ),
            predictor_name: self.name().to_string(),
        })
    }

    fn try_dominant(&self, dominant: WorkloadType, sample_count: usize) -> Option<Prediction> {
        if dominant == WorkloadType::Unknown {
            return None;
        }
        let confidence = 0.5;
        if confidence < self.min_confidence {
            return None;
        }
        Some(Prediction {
            predicted_workload: dominant,
            confidence,
            horizon_ms: self.horizon_ms,
            reason: format!(
                "dominant workload from {} samples: {:?}",
                sample_count, dominant
            ),
            predictor_name: self.name().to_string(),
        })
    }
}

impl Default for ShortHorizonPredictor {
    fn default() -> Self {
        Self::with_defaults()
    }
}

impl IPredictor for ShortHorizonPredictor {
    fn predict(&self, input: &PredictionInput) -> ContractResult<Prediction> {
        let memory = input.memory.as_ref().ok_or_else(|| {
            ContractError::AiInferenceFailed("no memory context for prediction".to_string())
        })?;

        if memory.sample_count < self.min_samples {
            return Err(ContractError::AiInferenceFailed(format!(
                "insufficient samples: {} < {}",
                memory.sample_count, self.min_samples
            )));
        }

        // Explicit application patterns outrank system-wide history when a
        // registered task identifies the app; absent identity never guesses.
        if let (Some(app), Some(timestamp_unix_ms)) =
            (&input.application_id, input.snapshot.collected_at_unix_ms)
        {
            let hour = ((timestamp_unix_ms / 3_600_000) % 24) as u8;
            if let Some(pattern) = memory.application_patterns.iter().find(|pattern| {
                &pattern.application_id == app
                    && pattern.hour_utc == hour
                    && pattern.sample_count >= self.min_samples
                    && pattern.confidence >= self.min_confidence.max(0.65)
            }) {
                return Ok(Prediction {
                    predicted_workload: pattern.dominant_workload,
                    confidence: pattern.confidence,
                    horizon_ms: self.horizon_ms,
                    reason: format!(
                        "registered application UTC hour {} pattern from {} samples",
                        hour, pattern.sample_count
                    ),
                    predictor_name: self.name().to_string(),
                });
            }
        }

        // 1. 一致性
        if let Some(pred) = self.try_consistent(&memory.recent_workloads, memory.sample_count) {
            return Ok(pred);
        }

        if let Some(timestamp_unix_ms) = input.snapshot.collected_at_unix_ms {
            let hour = ((timestamp_unix_ms / 3_600_000) % 24) as u8;
            if let Some(pattern) = memory.hourly_patterns.iter().find(|pattern| {
                pattern.hour_utc == hour
                    && pattern.sample_count >= self.min_samples
                    && pattern.confidence >= self.min_confidence.max(0.65)
            }) {
                return Ok(Prediction {
                    predicted_workload: pattern.dominant_workload,
                    confidence: pattern.confidence,
                    horizon_ms: self.horizon_ms,
                    reason: format!(
                        "UTC hour {} pattern from {} samples",
                        hour, pattern.sample_count
                    ),
                    predictor_name: self.name().to_string(),
                });
            }
        }

        // 2. 多数
        if let Some(dominant) = memory.dominant_workload {
            if let Some(pred) = self.try_dominant(dominant, memory.sample_count) {
                return Ok(pred);
            }
        }

        Err(ContractError::AiInferenceFailed(
            "no dominant or consistent workload to predict".to_string(),
        ))
    }

    fn name(&self) -> &'static str {
        "short_horizon"
    }

    fn horizon_ms(&self) -> u64 {
        self.horizon_ms
    }
}
