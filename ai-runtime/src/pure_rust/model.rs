use serde::{Deserialize, Serialize};

/// Weight table.
///
/// JSON format:
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
    /// Verify the consistency of weight structures.
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
            if row.iter().any(|weight| !weight.is_finite()) {
                return Err(format!("weights[{}] contains a non-finite value", i));
            }
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

    /// Dot-product and stable softmax inference.
    ///
    /// Returns `(label, score)` in descending order of fractions.
    pub fn infer(&self, features: &[f32]) -> Result<Vec<(String, f32)>, String> {
        if features.iter().any(|feature| !feature.is_finite()) {
            return Err("features contain a non-finite value".to_string());
        }
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

        if scores.iter().any(|(_, score)| !score.is_finite()) {
            return Err("non-finite inference logit".to_string());
        }

        // softmax (value stable)
        let max = scores
            .iter()
            .map(|(_, s)| *s)
            .fold(f32::NEG_INFINITY, f32::max);
        let exps: Vec<f32> = scores.iter().map(|(_, s)| (s - max).exp()).collect();
        let sum: f32 = exps.iter().sum();
        if !sum.is_finite() || sum <= 0.0 {
            return Err("softmax denominator is not finite and positive".to_string());
        }

        for ((_, score), exp) in scores.iter_mut().zip(exps.iter()) {
            *score = exp / sum;
        }

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        Ok(scores)
    }
}

#[cfg(test)]
mod tests {
    use super::ModelWeights;

    fn model() -> ModelWeights {
        ModelWeights {
            schema_version: "1.0".into(),
            name: "finite-test".into(),
            version: "1.0.0".into(),
            feature_dim: 1,
            labels: vec!["a".into(), "b".into()],
            weights: vec![vec![2.0], vec![-2.0]],
        }
    }

    #[test]
    fn non_finite_features_are_rejected() {
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(model().infer(&[value]).is_err());
        }
    }

    #[test]
    fn non_finite_weights_are_rejected() {
        let mut model = model();
        model.weights[0][0] = f32::INFINITY;
        assert!(model.validate().is_err());
    }

    #[test]
    fn overflowing_logits_are_rejected() {
        assert!(model().infer(&[f32::MAX]).is_err());
    }

    #[test]
    fn finite_softmax_is_normalized_and_sorted() {
        let scores = model().infer(&[100.0]).unwrap();
        assert_eq!(scores[0].0, "a");
        assert!(scores
            .iter()
            .all(|(_, score)| score.is_finite() && (0.0..=1.0).contains(score)));
        assert!((scores.iter().map(|(_, score)| score).sum::<f32>() - 1.0).abs() < 1e-6);
        assert!(model().infer(&[]).is_err());
    }
}
