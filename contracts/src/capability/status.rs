use serde::{Deserialize, Serialize};

/// 组件 / 能力在特定平台上的支持状态。
///
/// 语义：
/// - `Supported`：完整支持，可直接使用。
/// - `Partial`：部分支持，例如只能设置上限不能设置下限。
/// - `ReadOnly`：只能读取，不能控制。
/// - `Conditional`：依赖外部条件（权限、硬件、运行时），需要运行时检查。
/// - `Unsupported`：当前平台不支持。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityStatus {
    Supported,
    Partial,
    ReadOnly,
    Conditional,
    Unsupported,
}

impl CapabilityStatus {
    /// 该状态是否可用于控制类操作。
    pub fn is_controllable(self) -> bool {
        matches!(self, Self::Supported | Self::Partial | Self::Conditional)
    }

    /// 该状态是否只读。
    pub fn is_read_only(self) -> bool {
        matches!(self, Self::ReadOnly)
    }
}
