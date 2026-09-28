use serde::{Deserialize, Serialize};

use super::workload::{InteractionLevel, WorkloadType};

/// Uniform snapshot of the context of the current system.
///
/// Different from ComputerSnapshot:
/// - ComputerSnapshot is "what hardware is doing."
/// - ContextSnapshot is "What the user is doing, what the application is doing."
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ContextSnapshot {
    /// Collect time, Unix epoch millisecond.
    pub timestamp_unix_ms: Option<u64>,

    /// Foreground applications such as "Code", "chrome".
    pub foreground_app: Option<String>,

    /// Foreground window title (if platform supports).
    pub foreground_window_title: Option<String>,

    /// Foreground process name.
    pub foreground_process_name: Option<String>,

    /// List of currently active applications (reduced, maximum N).
    pub active_apps: Vec<String>,

    /// Recognized workload type.
    pub workload_type: WorkloadType,

    /// User interactive level.
    pub interaction_level: InteractionLevel,

    /// Recognize confidence, 0.0-1.0. The rule provider gives a rough confidence.
    pub confidence: f32,
}
