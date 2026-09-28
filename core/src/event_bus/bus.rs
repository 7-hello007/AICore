use std::sync::{Arc, RwLock};

use aicore_contracts::events::{
    ai_suggestion_created::AiSuggestionCreated, context_updated::ContextUpdated,
    execution_completed::ExecutionCompleted, policy_rejected::PolicyRejected,
    policy_updated::PolicyUpdated, runtime_mode_changed::RuntimeModeChanged,
    system_state_updated::SystemStateUpdated, task_submitted::TaskSubmitted,
};

#[derive(Debug, Clone)]
// Event variants vary considerably (SystemStateUpdated contains complete ComputerSnapshot).
// EventBus is currently transmitted by reference (public (&self, event: & Event), without copying a message.
// Therefore, no box. If EventBus is subsequently converted to value-based distribution, it will need to be reassessed.
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
        // Subscriber callbacks may subscribe/unsubscribe; never call them
        // while holding the registry lock. Changes apply to the next publish.
        let subscribers = match self.subscribers.read() {
            Ok(subs) => subs.clone(),
            Err(_) => return,
        };
        for sub in subscribers {
            sub.handle(event);
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
