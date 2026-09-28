use serde::{Deserialize, Serialize};

/// Compatibility level.
///
/// - L0: Zero change software is transparent.
/// - L1: Optional sensory integration (applying active declarations).
/// - L2: Advanced primary integration (application depth access).
///
/// ** Peer relationships**: L0 / L1 / L2 are not dependent on each other and delete any without prejudice to others.
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
