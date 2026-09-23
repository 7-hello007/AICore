use serde::{Deserialize, Serialize};

use super::workload::{InteractionLevel, WorkloadType};

/// 当前系统上下文的统一快照。
///
/// 与 ComputerSnapshot 的区别：
/// - ComputerSnapshot 是“硬件在做什么”。
/// - ContextSnapshot 是“用户在做什么、应用在做什么”。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ContextSnapshot {
    /// 采集时间，Unix epoch 毫秒。
    pub timestamp_unix_ms: Option<u64>,

    /// 前台应用名，例如 "Code"、"chrome"。
    pub foreground_app: Option<String>,

    /// 前台窗口标题（如果平台支持）。
    pub foreground_window_title: Option<String>,

    /// 前台进程名。
    pub foreground_process_name: Option<String>,

    /// 当前活跃的应用列表（去重，最多 N 个）。
    pub active_apps: Vec<String>,

    /// 识别出的 workload 类型。
    pub workload_type: WorkloadType,

    /// 用户交互级别。
    pub interaction_level: InteractionLevel,

    /// 识别置信度，0.0 - 1.0。规则分类器可给出粗略置信度。
    pub confidence: f32,
}
