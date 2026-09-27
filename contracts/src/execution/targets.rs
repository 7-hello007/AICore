use serde::{Deserialize, Serialize};

/// 策略执行的目标进程集合。
///
/// Core 不直接决定哪些 PID 是“前台/后台”——那是 Context Engine 或
/// 上层驱动的职责。Executor 只消费这个集合。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ExecutionTargets {
    /// Application-bound preparation may change only its target processes.
    /// Default false preserves existing full-policy execution.
    #[serde(default)]
    pub process_only: bool,
    /// 前台进程 PID；接收策略的整体 CPU 优先级。
    pub foreground_pids: Vec<u32>,
    /// 后台进程 PID；当策略要求节流时，被降为 Background 优先级。
    pub background_pids: Vec<u32>,
}
