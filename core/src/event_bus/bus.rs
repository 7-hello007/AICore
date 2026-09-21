use std::sync::{Arc, RwLock};

use aicore_contracts::events::{
    ai_suggestion_created::AiSuggestionCreated, context_updated::ContextUpdated,
    execution_completed::ExecutionCompleted, policy_rejected::PolicyRejected,
    policy_updated::PolicyUpdated, runtime_mode_changed::RuntimeModeChanged,
    system_state_updated::SystemStateUpdated, task_submitted::TaskSubmitted,
};

#[derive(Debug, Clone)]
// Event 变体大小差异较大（SystemStateUpdated 包含完整 ComputerSnapshot）。
// 当前 EventBus 用引用传递（publish(&self, event: &Event)），不做 clone 拷贝，
// 因此不 box。若后续 EventBus 改为按值分发，需重新评估。
#[allow(clippy::large_enum_variant)]
pub enum Event {
    SystemStateUpdated(SystemStateUpdated),
    ContextUpdated(ContextUpdated),
    PolicyUpdated(PolicyUpdated),
    PolicyRejected(PolicyRejected),
    ExecutionCompleted(ExecutionCompleted),
    RuntimeModeChanged(RuntimeModeChanged),
    TaskSubmitted(TaskSubmitted),
    AiSuggestionCreated(AiSuggestionCreated),
}

impl Event {
    pub fn name(&self) -> &'static str {
        match self {
            Event::SystemStateUpdated(_) => "SystemStateUpdated",
            Event::ContextUpdated(_) => "ContextUpdated",
            Event::PolicyUpdated(_) => "PolicyUpdated",
            Event::PolicyRejected(_) => "PolicyRejected",
            Event::ExecutionCompleted(_) => "ExecutionCompleted",
            Event::RuntimeModeChanged(_) => "RuntimeModeChanged",
            Event::TaskSubmitted(_) => "TaskSubmitted",
            Event::AiSuggestionCreated(_) => "AiSuggestionCreated",
        }
    }
}

pub trait EventSubscriber: Send + Sync {
    fn handle(&self, event: &Event);
    fn name(&self) -> &'static str;
}

pub struct EventBus {
    subscribers: RwLock<Vec<Arc<dyn EventSubscriber>>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            subscribers: RwLock::new(Vec::new()),
        }
    }

    pub fn subscribe(&self, subscriber: Arc<dyn EventSubscriber>) {
        if let Ok(mut subs) = self.subscribers.write() {
            subs.push(subscriber);
        }
    }

    pub fn unsubscribe(&self, name: &str) -> bool {
        match self.subscribers.write() {
            Ok(mut subs) => {
                let before = subs.len();
                subs.retain(|sub| sub.name() != name);
                subs.len() != before
            },
            Err(_) => false,
        }
    }

    pub fn publish(&self, event: &Event) {
        if let Ok(subs) = self.subscribers.read() {
            for sub in subs.iter() {
                sub.handle(event);
            }
        }
    }

    pub fn subscriber_count(&self) -> usize {
        self.subscribers.read().map(|s| s.len()).unwrap_or(0)
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
