use serde::{Deserialize, Serialize};

/// Targeting processes for strategy implementation.
///
/// Core does not directly determine which PID is "foreground/background" -- that's Context Engineering or
/// the upper layer. The executor only consumes this collection.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ExecutionTargets {
    /// Application-bound preparation may change only its target processes.
    /// Default false preserves existing full-policy execution.
    #[serde(default)]
    pub process_only: bool,
    /// Foreground process PID; overall CPU priority for reception strategy.
    pub foreground_pids: Vec<u32>,
    /// Background process PID; downgraded to Background priority when the strategy calls for throttle.
    pub background_pids: Vec<u32>,
}
