use serde::{Deserialize, Serialize};

use super::features::FEATURE_DIM;

/// 策略建议类型。
///
/// 每个类型对应一组权重；Provider 选择点积最高的。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyIntent {
    /// 提升 CPU 优先级。
    BoostCpu,
    /// 提升 GPU 优先级。
    BoostGpu,
    /// 提升 NPU 优先级。
    BoostNpu,
    /// 降低后台任务。
    ThrottleBackground,
    /// 省电。
    PowerSave,
    /// 保持原策略。
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

/// 权重表。
///
/// 结构：每个 intent 一组 `[f32; FEATURE_DIM]` 权重。
/// 可从 JSON 加载 / 保存。
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
    /// 默认权重。经验值，非最优。
    ///
    /// 特征顺序：`[cpu, gpu, npu, mem, interactive, passive, idle, has_memory]`
    pub fn default_weights() -> Self {
        // 目标：在 Interactive + 高 CPU 时偏好 BoostCpu，高 NPU 时偏好 BoostNpu，等等。
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

    /// 查找某个 intent 的权重。
    pub fn weights_for(&self, intent: PolicyIntent) -> Option<[f32; FEATURE_DIM]> {
        let entry = self.entries.iter().find(|e| e.intent == intent)?;
        if entry.weights.len() != FEATURE_DIM {
            return None;
        }
        let mut arr = [0.0f32; FEATURE_DIM];
        arr.copy_from_slice(&entry.weights);
        Some(arr)
    }

    /// 校验：每个 intent 恰好一条，维度正确。
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

    /// 从 JSON 加载。
    pub fn from_json(s: &str) -> Result<Self, String> {
        let table: Self = serde_json::from_str(s).map_err(|e| format!("parse weights: {}", e))?;
        table.validate()?;
        Ok(table)
    }

    /// 保存到 JSON。
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| format!("serialize weights: {}", e))
    }
}

impl Default for WeightTable {
    fn default() -> Self {
        Self::default_weights()
    }
}
