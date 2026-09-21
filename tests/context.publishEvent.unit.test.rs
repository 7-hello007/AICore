use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use aicore_contracts::{
    context::{snapshot::ContextSnapshot, workload::WorkloadType},
    events::context_updated::ContextUpdated,
};
use aicore_core::event_bus::{Event, EventBus, EventSubscriber};

struct CountingSubscriber {
    count: AtomicUsize,
}

impl EventSubscriber for CountingSubscriber {
    fn handle(&self, event: &Event) {
        if matches!(event, Event::ContextUpdated(_)) {
            self.count.fetch_add(1, Ordering::SeqCst);
        }
    }
    fn name(&self) -> &'static str {
        "counting"
    }
}

#[test]
fn bus_delivers_context_event() {
    let bus = EventBus::new();
    let sub = Arc::new(CountingSubscriber {
        count: AtomicUsize::new(0),
    });
    bus.subscribe(sub.clone());

    assert_eq!(bus.subscriber_count(), 1);

    let event = Event::ContextUpdated(ContextUpdated {
        version: "1.0".to_string(),
        timestamp_unix_ms: 0,
        context: ContextSnapshot {
            workload_type: WorkloadType::Development,
            ..Default::default()
        },
    });
    bus.publish(&event);

    assert_eq!(sub.count.load(Ordering::SeqCst), 1);
}

#[test]
fn bus_without_subscribers_is_noop() {
    let bus = EventBus::new();
    let event = Event::ContextUpdated(ContextUpdated {
        version: "1.0".to_string(),
        timestamp_unix_ms: 0,
        context: ContextSnapshot::default(),
    });
    bus.publish(&event); // 不应 panic
    assert_eq!(bus.subscriber_count(), 0);
}
