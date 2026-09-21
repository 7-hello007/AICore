use serde::{Deserialize, Serialize};

/// 兼容等级。
///
/// - L0：零改动软件透明适配。
/// - L1：可选感知集成（应用主动声明）。
/// - L2：原生高级集成（应用深度接入）。
///
/// **平级关系**：L0 / L1 / L2 互不依赖，删除任一不影响其他。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TierLevel {
    L0,
    L1,
    L2,
}

impl TierLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::L0 => "L0",
            Self::L1 => "L1",
            Self::L2 => "L2",
        }
    }
}
