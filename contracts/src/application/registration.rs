use serde::{Deserialize, Serialize};

use super::id::ApplicationId;

/// 应用注册信息。
///
/// 在 AICore 中注册后，应用才有资格提交 TaskContract。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApplicationRegistration {
    pub application_id: ApplicationId,
    pub name: String,
    pub version: String,
    pub registered_at_unix_secs: u64,
    /// 应用声称的能力，例如 ["gpu.compute", "npu.inference"]。
    /// AICore 可据此做更精确的能力匹配。
    pub capabilities: Vec<String>,
}
