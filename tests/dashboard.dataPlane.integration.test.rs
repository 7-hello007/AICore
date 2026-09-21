//! Dashboard 数据面：TickOutcome 字段完整性 + TickSummary 映射 + 序列化。

use aicore_contracts::{
    context::{
        snapshot::ContextSnapshot,
        workload::{InteractionLevel, WorkloadType},
    },
    execution::status::ExecutionOverallStatus,
    policy::{
        priority::{PolicyPriority, PolicySource},
        resource_policy::{
            BackgroundPolicy, CpuPolicy, GpuPolicy, MemoryPolicy, NpuPolicy, PowerMode,
            PowerPolicy, ResourcePolicy,
        },
    },
    runtime::{
        daemon_status::TickSummary,
        mode::RuntimeMode,
        route::{RuntimeRoute, RuntimeRouteSource},
        tick::TickOutcome,
    },
};

fn full_outcome() -> TickOutcome {
    let ctx = ContextSnapshot {
        workload_type: WorkloadType::Development,
        interaction_level: InteractionLevel::Interactive,
        confidence: 0.9,
        ..Default::default()
    };
    let policy = ResourcePolicy {
        policy_id: "p-1".to_string(),
        priority: PolicyPriority::High,
        source: PolicySource::RuleBased,
        reason: "test".to_string(),
        cpu: CpuPolicy {
            priority: PolicyPriority::High,
            affinity_cores: None,
        },
        gpu: GpuPolicy {
            priority: PolicyPriority::Normal,
            workload_preference: None,
        },
        npu: NpuPolicy {
            priority: PolicyPriority::Normal,
            preferred: false,
        },
        memory: MemoryPolicy {
            priority: PolicyPriority::Normal,
            max_usage_percent: None,
        },
        power: PowerPolicy {
            mode: PowerMode::Balanced,
        },
        background: BackgroundPolicy {
            throttle: true,
            max_cpu_percent: Some(20.0),
        },
        ..Default::default()
    };
    let route = RuntimeRoute {
        route_id: "r-1".to_string(),
        workload_type: WorkloadType::Development,
        source: RuntimeRouteSource::Cpu,
        applied: true,
        reason: "cpu".to_string(),
        confidence: 0.9,
        decided_at_unix_secs: 1_700_000_000,
    };

    TickOutcome {
        tick_id: 1,
        trace_id: "t-1".to_string(),
        timestamp_unix_ms: 1_700_000_000,
        duration_ms: 15,
        success: true,
        mode_after: RuntimeMode::Normal,
        workload_type: WorkloadType::Development,
        policy_id: Some("p-1".to_string()),
        policy_source: Some(PolicySource::RuleBased),
        validation_accepted: true,
        execution_status: Some(ExecutionOverallStatus::AllSucceeded),
        message: "ok".to_string(),
        predicted_workload: Some(WorkloadType::Development),
        prediction_confidence: Some(0.8),
        runtime_route_source: Some("cpu".to_string()),
        runtime_binding_executed: Some(true),
        runtime_binding_reason: Some("binder accepted".to_string()),
        cpu_usage_percent: Some(60.0),
        gpu_usage_percent: Some(10.0),
        npu_usage_percent: Some(0.0),
        memory_usage_percent: Some(50.0),
        current_context_json: serde_json::to_string(&ctx).ok(),
        current_policy_json: serde_json::to_string(&policy).ok(),
        runtime_route_json: serde_json::to_string(&route).ok(),
        ai_runtime_health_json: Some(
            r#"{"runtime_name":"mock","healthy":true,"loaded_models":1}"#.to_string(),
        ),
    }
}

#[test]
fn outcome_has_all_dashboard_fields() {
    let o = full_outcome();
    assert_eq!(o.cpu_usage_percent, Some(60.0));
    assert_eq!(o.gpu_usage_percent, Some(10.0));
    assert_eq!(o.npu_usage_percent, Some(0.0));
    assert_eq!(o.memory_usage_percent, Some(50.0));
    assert!(o.current_context_json.is_some());
    assert!(o.current_policy_json.is_some());
    assert!(o.runtime_route_json.is_some());
    assert!(o.ai_runtime_health_json.is_some());
}

#[test]
fn tick_summary_maps_new_fields() {
    let o = full_outcome();
    let s = TickSummary::from_outcome(&o);

    assert_eq!(s.runtime_route_source.as_deref(), Some("cpu"));
    assert_eq!(s.runtime_binding_executed, Some(true));
    assert_eq!(s.cpu_usage_percent, Some(60.0));
    assert_eq!(s.gpu_usage_percent, Some(10.0));
    assert_eq!(s.npu_usage_percent, Some(0.0));
    assert_eq!(s.memory_usage_percent, Some(50.0));
    assert!(s.current_context_json.is_some());
    assert!(s.current_policy_json.is_some());
    assert!(s.runtime_route_json.is_some());
    assert!(s.ai_runtime_health_json.is_some());
}

#[test]
fn summary_serde_roundtrip() {
    let s = TickSummary::from_outcome(&full_outcome());
    let json = serde_json::to_string(&s).unwrap();
    let back: TickSummary = serde_json::from_str(&json).unwrap();
    assert_eq!(back, s);
}

#[test]
fn summary_default_has_all_none() {
    let s = TickSummary::default();
    assert!(s.cpu_usage_percent.is_none());
    assert!(s.current_context_json.is_none());
    assert!(s.runtime_route_json.is_none());
    assert!(s.ai_runtime_health_json.is_none());
}

#[test]
fn json_roundtrip_for_route_survives() {
    let o = full_outcome();
    let route_json = o.runtime_route_json.as_ref().unwrap();
    let route: RuntimeRoute = serde_json::from_str(route_json).unwrap();
    assert_eq!(route.source, RuntimeRouteSource::Cpu);
    assert!(route.applied);
}

#[test]
fn json_roundtrip_for_context_survives() {
    let o = full_outcome();
    let ctx_json = o.current_context_json.as_ref().unwrap();
    let ctx: ContextSnapshot = serde_json::from_str(ctx_json).unwrap();
    assert_eq!(ctx.workload_type, WorkloadType::Development);
}
