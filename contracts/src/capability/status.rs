use serde::{Deserialize, Serialize};

/// Component or capability support status on a specific platform.
///
/// Semantics:
/// - `Supported`: Full support, directly available.
/// - `Partial`: Partially supported, for example, by setting a ceiling without setting a floor.
/// - `ReadOnly`: readable, uncontrollable.
/// `Conditional`: Reliance on external conditions (permissions, hardware, runtime availability) requires running-time checks.
/// - `Unsupported`: The current platform is not supported.
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
    /// Whether this state can be used for control class operations.
    pub fn is_controllable(self) -> bool {
        matches!(self, Self::Supported | Self::Partial | Self::Conditional)
    }

    /// Whether the state is read-only.
    pub fn is_read_only(self) -> bool {
        matches!(self, Self::ReadOnly)
    }
}
