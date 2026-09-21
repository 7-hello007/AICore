use serde::{Deserialize, Serialize};

/// 权重表。
///
/// JSON 格式：
/// ```json
/// {
///   "schema_version": "1.0",
///   "name": "policy_classifier_v1",
///   "version": "1.0.0",
///   "feature_dim": 4,
///   "labels": ["boost_cpu", "power_save", ...],
///   "weights": [[0.6, 0.0, 0.0, 0.0], ...]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelWeights {
    pub schema_version: String,
    pub name: String,
    pub version: String,
    pub feature_dim: usize,
    pub labels: Vec<String>,
    pub weights: Vec<Vec<f32>>,
}

impl ModelWeights {
    /// 校验权重结构一致性。
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("name is empty".to_string());
        }
        if self.version.trim().is_empty() {
            return Err("version is empty".to_string());
        }
        if self.schema_version.trim().is_empty() {
            return Err("schema_version is empty".to_string());
        }
        if self.feature_dim == 0 {
            return Err("feature_dim must be > 0".to_string());
        }
        if self.labels.is_empty() {
            return Err("labels must not be empty".to_string());
        }
        if self.labels.len() != self.weights.len() {
            return Err(format!(
                "labels ({}) and weights ({}) length mismatch",
                self.labels.len(),
                self.weights.len()
            ));
        }
        for (i, row) in self.weights.iter().enumerate() {
            if row.len() != self.feature_dim {
                return Err(format!(
                    "weights[{}] length {} != feature_dim {}",
                    i,
                    row.len(),
                    self.feature_dim
                ));
            }
        }
        Ok(())
    }

    /// 点积 + softmax 推理。
    ///
    /// 返回按分数降序排列的 `(label, score)`。
    pub fn infer(&self, features: &[f32]) -> Result<Vec<(String, f32)>, String> {
        if features.len() != self.feature_dim {
            return Err(format!(
                "feature dimension {} != expected {}",
                features.len(),
                self.feature_dim
            ));
        }

        let mut scores: Vec<(String, f32)> = self
            .labels
            .iter()
            .zip(self.weights.iter())
            .map(|(label, row)| {
                let s: f32 = row.iter().zip(features.iter()).map(|(a, b)| a * b).sum();
                (label.clone(), s)
            })
            .collect();

        // softmax（数值稳定）
        let max = scores
            .iter()
            .map(|(_, s)| *s)
            .fold(f32::NEG_INFINITY, f32::max);
        let exps: Vec<f32> = scores.iter().map(|(_, s)| (s - max).exp()).collect();
        let sum: f32 = exps.iter().sum();
        if sum <= 0.0 {
            return Err("softmax denominator is zero".to_string());
        }

        for ((_, score), exp) in scores.iter_mut().zip(exps.iter()) {
            *score = exp / sum;
        }

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        Ok(scores)
    }
}
