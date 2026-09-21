use serde::{Deserialize, Serialize};

use crate::policy::resource_policy::PowerMode;

/// 用户个人配置。
///
/// 隐私约定（参照 DATA_PRIVACY.md）：
/// - history_enabled 默认 false：长期历史默认不记录。
/// - profile_enabled 默认 false：个人模型默认不生成。
/// - cloud_inference_allowed 默认 false：默认禁止云端推理。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserProfile {
    pub version: String,
    pub history_enabled: bool,
    pub profile_enabled: bool,
    pub retention_days: u32,
    pub preferred_power_mode: Option<PowerMode>,
    pub cloud_inference_allowed: bool,
    pub updated_at_unix_secs: u64,
}

impl Default for UserProfile {
    fn default() -> Self {
        Self {
            version: "1".to_string(),
            history_enabled: false,
            profile_enabled: false,
            retention_days: 7,
            preferred_power_mode: None,
            cloud_inference_allowed: false,
            updated_at_unix_secs: 0,
        }
    }
}

impl UserProfile {
    /// 是否可以记录历史。
    pub fn can_record(&self) -> bool {
        self.history_enabled
    }

    /// 是否可以使用个人模型。
    pub fn can_personalize(&self) -> bool {
        self.profile_enabled
    }

    /// 是否允许云端推理。
    pub fn can_use_cloud(&self) -> bool {
        self.cloud_inference_allowed
    }
}
