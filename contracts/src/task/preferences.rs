use serde::{Deserialize, Serialize};

/// 应用的延迟敏感度。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LatencySensitivity {
    Low,
    Medium,
    High,
    Critical,
}

/// 应用的功耗偏好。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PowerPreference {
    PowerSave,
    Balanced,
    Performance,
}

/// 应用的数据处理约束。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyRequirement {
    /// 明确要求本地处理。
    LocalOnly,
    /// 允许云端处理（需用户全局开启）。
    CloudAllowed,
    /// 未声明；由 AICore 默认策略决定。
    Unspecified,
}

/// 硬件需求强度。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HardwareRequirement {
    NotNeeded,
    Optional,
    Preferred,
    Required,
}
