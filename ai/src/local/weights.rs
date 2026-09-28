use serde::{Deserialize, Serialize};

use super::features::FEATURE_DIM;

/// Type of strategy proposal.
///
/// Each type corresponds to a set of weights; Provider select points with the highest volume.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyIntent {
    /// Raise CPU priority.
    BoostCpu,
    /// Raise GPU priority.
    BoostGpu,
    /// Raise NPU priority.
    BoostNpu,
    /// Lower background-task priority.
    ThrottleBackground,
    /// Power conservation.
    PowerSave,
    /// Stay with the strategy.
    KeepBase,
}

impl PolicyIntent {
    pub const ALL: [PolicyIntent; 6] = [
        PolicyIntent::BoostCpu,
        PolicyIntent::BoostGpu,
        PolicyIntent::BoostNpu,
        PolicyIntent::ThrottleBackground,
        PolicyIntent::PowerSave,
        PolicyIntent::KeepBase,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            PolicyIntent::BoostCpu => "boost_cpu",
            PolicyIntent::BoostGpu => "boost_gpu",
            PolicyIntent::BoostNpu => "boost_npu",
            PolicyIntent::ThrottleBackground => "throttle_background",
            PolicyIntent::PowerSave => "power_save",
            PolicyIntent::KeepBase => "keep_base",
        }
    }
}

/// Weight table.
///
/// Structure: A set of `[f32; FEATURE_DIM]` weights per intent.
/// Load/ Save from JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeightTable {
    pub version: String,
    pub entries: Vec<WeightEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeightEntry {
    pub intent: PolicyIntent,
    pub weights: Vec<f32>,
}

impl WeightTable {
    /// Default weights. Empirical values, not optimal.
    ///
    /// Feature order: `[cpu, gpu, npu, mem, interactive, passive, idle, has_memory]`
    pub fn default_weights() -> Self {
        // Target: At Interact + High CPU Time preferences BoostCpuHigh NPU Time preferences BoostNpuWait.
        let entries = vec![
            WeightEntry {
                intent: PolicyIntent::BoostCpu,
                weights: vec![0.6, 0.0, 0.0, 0.0, 0.8, 0.0, 0.0, 0.0],
            },
            WeightEntry {
                intent: PolicyIntent::BoostGpu,
                weights: vec![0.0, 0.7, 0.0, 0.0, 0.8, 0.0, 0.0, 0.0],
            },
            WeightEntry {
                intent: PolicyIntent::BoostNpu,
                weights: vec![0.0, 0.0, 0.8, 0.0, 0.8, 0.0, 0.0, 0.0],
            },
            WeightEntry {
                intent: PolicyIntent::ThrottleBackground,
                weights: vec![0.4, 0.4, 0.0, 0.4, 0.0, 0.6, 0.0, 0.0],
            },
            WeightEntry {
                intent: PolicyIntent::PowerSave,
                weights: vec![0.0, 0.0, 0.0, 0.2, 0.0, 0.0, 0.9, 0.3],
            },
            WeightEntry {
                intent: PolicyIntent::KeepBase,
                weights: vec![0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2],
            },
        ];

        Self {
            version: "1.0.0".to_string(),
            entries,
        }
    }

    /// Finds the weight of an intent.
    pub fn weights_for(&self, intent: PolicyIntent) -> Option<[f32; FEATURE_DIM]> {
        let entry = self.entries.iter().find(|e| e.intent == intent)?;
        if entry.weights.len() != FEATURE_DIM {
            return None;
        }
        let mut arr = [0.0f32; FEATURE_DIM];
        arr.copy_from_slice(&entry.weights);
        Some(arr)
    }

    /// Validation: Every intent exactly one dimension is correct.
    pub fn validate(&self) -> Result<(), String> {
        for intent in PolicyIntent::ALL {
            let count = self.entries.iter().filter(|e| e.intent == intent).count();
            if count != 1 {
                return Err(format!(
                    "intent {:?} must have exactly one entry, got {}",
                    intent, count
                ));
            }
        }
        for e in &self.entries {
            if e.weights.len() != FEATURE_DIM {
                return Err(format!(
                    "intent {:?} weights length {} != {}",
                    e.intent,
                    e.weights.len(),
                    FEATURE_DIM
                ));
            }
        }
        Ok(())
    }

    /// Load from JSON.
    pub fn from_json(s: &str) -> Result<Self, String> {
        let table: Self = serde_json::from_str(s).map_err(|e| format!("parse weights: {}", e))?;
        table.validate()?;
        Ok(table)
    }

    /// Save to JSON.
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| format!("serialize weights: {}", e))
    }
}

impl Default for WeightTable {
    fn default() -> Self {
        Self::default_weights()
    }
}
