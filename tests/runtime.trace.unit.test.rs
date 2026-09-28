//! Authentication TraceContext and TickOutcome Yes. trace_id Integration.

use aicore_contracts::runtime::trace::TraceContext;

#[test]
fn trace_id_includes_tick_id() {
    let t = TraceContext::new(42);
    assert!(t.trace_id.contains("42"));
    assert!(t.trace_id.starts_with("trace-"));
    assert_eq!(t.tick_id, 42);
    assert!(t.started_at_unix_ms > 0);
}

#[test]
fn trace_ids_are_unique_across_ticks() {
    let a = TraceContext::new(1);
    let b = TraceContext::new(2);
    assert_ne!(a.trace_id, b.trace_id);
}

#[test]
fn trace_id_serializes() {
    let t = TraceContext::new(7);
    let json = serde_json::to_string(&t).expect("serialize");
    assert!(json.contains("trace_id"));
    assert!(json.contains("tick_id"));

    let back: TraceContext = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, t);
}

#[test]
fn elapsed_ms_is_non_negative() {
    let t = TraceContext::new(0);
    std::thread::sleep(std::time::Duration::from_millis(2));
    let elapsed = t.elapsed_ms();
    assert!(elapsed < 10_000, "elapsed_ms={} seems wrong", elapsed);
}
