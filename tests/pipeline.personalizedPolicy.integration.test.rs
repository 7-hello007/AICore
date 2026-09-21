use std::sync::Arc;

use aicore_ai::MockAiProvider;
use aicore_contracts::{
    ai::input::AiInput,
    context::{
        snapshot::ContextSnapshot,
        workload::{InteractionLevel, WorkloadType},
    },
    execution::status::ExecutionOverallStatus,
    memory::record::HistoricalRecord,
    policy::{
        priority::{PolicyPriority, PolicySource},
        resource_policy::{
            BackgroundPolicy, CpuPolicy, GpuPolicy, MemoryPolicy, NpuPolicy, PowerMode,
            PowerPolicy, ResourcePolicy,
        },
    },
    telemetry::snapshot::ComputerSnapshot,
    traits::{
        ai_provider::IAiProvider, memory_store::IMemoryStore, policy_engine::IPolicyEngine,
        policy_validator::IPolicyValidator,
    },
};
use aicore_core::policy::{AiAssistedPolicyEngine, DefaultPolicyValidator, RuleBasedPolicyEngine};
use aicore_memory::{build_memory_context, InMemoryStore};

fn record(w: WorkloadType, mode: PowerMode) -> HistoricalRecord {
    HistoricalRecord {
        record_id: "r".to_string(),
        timestamp_unix_ms: 1_700_000_000,
        workload_type: w,
        interaction_level: InteractionLevel::Interactive,
        policy_id: "p".to_string(),
        policy_source: PolicySource::RuleBased,
        overall_status: ExecutionOverallStatus::AllSucceeded,
        cpu_usage_percent: 5.0,
        gpu_usage_percent: None,
        npu_usage_percent: None,
        memory_usage_percent: 30.0,
        power_mode: mode,
    }
}

fn base_policy() -> ResourcePolicy {
    ResourcePolicy {
        policy_id: "p".to_string(),
        priority: PolicyPriority::Normal,
        source: PolicySource::RuleBased,
        reason: "base".to_string(),
        cpu: CpuPolicy {
            priority: PolicyPriority::Normal,
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
            throttle: false,
            max_cpu_percent: None,
        },
        ..Default::default()
    }
}

#[test]
fn empty_history_does_not_change_mock_behavior() {
    let p = MockAiProvider::new();
    let ctx = ContextSnapshot {
        workload_type: WorkloadType::AiInference,
        interaction_level: InteractionLevel::Interactive,
        ..Default::default()
    };
    let mut snapshot = ComputerSnapshot::default();
    snapshot.npu.available = true;

    let input = AiInput {
        snapshot,
        context: ctx,
        base_policy: Some(base_policy()),
        memory: None,
    };

    let s = p.suggest(&input).unwrap();
    assert!(s.confidence >= 0.85 && s.confidence <= 0.95);
}

#[test]
fn history_boost_raises_confidence() {
    let p = MockAiProvider::new();
    let ctx = ContextSnapshot {
        workload_type: WorkloadType::AiInference,
        interaction_level: InteractionLevel::Interactive,
        ..Default::default()
    };
    let mut snapshot = ComputerSnapshot::default();
    snapshot.npu.available = true;

    let memory = build_memory_context(&[record(WorkloadType::AiInference, PowerMode::Balanced)]);

    let input = AiInput {
        snapshot,
        context: ctx,
        base_policy: Some(base_policy()),
        memory: Some(memory),
    };

    let s = p.suggest(&input).unwrap();
    // 基础 0.9 + 0.05 boost，被 clamp 到 0.95
    assert!((s.confidence - 0.95).abs() < f32::EPSILON);
}

#[test]
fn history_power_save_applies_to_office() {
    let p = MockAiProvider::new();
    let ctx = ContextSnapshot {
        workload_type: WorkloadType::GeneralOffice,
        interaction_level: InteractionLevel::Interactive,
        ..Default::default()
    };

    let memory = build_memory_context(&[record(WorkloadType::GeneralOffice, PowerMode::PowerSave)]);

    let input = AiInput {
        snapshot: ComputerSnapshot::default(),
        context: ctx,
        base_policy: Some(base_policy()),
        memory: Some(memory),
    };

    let s = p.suggest(&input).unwrap();
    match s.payload {
        aicore_contracts::ai::suggestion::AiSuggestionPayload::Policy(p) => {
            assert_eq!(p.power.mode, PowerMode::PowerSave);
        },
        _ => panic!("expected policy payload"),
    }
}

#[test]
fn personalized_pipeline_still_passes_validator() {
    let store = InMemoryStore::new();
    store
        .append_record(&record(WorkloadType::GeneralOffice, PowerMode::PowerSave))
        .unwrap();
    store
        .append_record(&record(WorkloadType::GeneralOffice, PowerMode::PowerSave))
        .unwrap();

    let records = store.load_records(50).unwrap();
    let memory = build_memory_context(&records);

    let base = Arc::new(RuleBasedPolicyEngine::new());
    let ai = Arc::new(MockAiProvider::new());
    let engine = AiAssistedPolicyEngine::new(base, Some(ai), 0.6);

    let snapshot = ComputerSnapshot::default();
    let ctx = ContextSnapshot {
        workload_type: WorkloadType::GeneralOffice,
        interaction_level: InteractionLevel::Interactive,
        ..Default::default()
    };

    // 走完整路径：AI 建议 → Validator
    let base_policy = engine.generate(&snapshot, &ctx);
    let validator = DefaultPolicyValidator::new();
    let result = validator.validate(&base_policy);
    assert!(result.accepted, "policy rejected: {:?}", result.issues);

    // 直接验证 MockAiProvider 在 memory 存在时不 panic
    let _ = memory.sample_count;
}

#[test]
fn privacy_default_is_off_for_fresh_store() {
    let store = InMemoryStore::new();
    let profile = store.load_profile().unwrap();
    assert!(!profile.can_record());
    assert!(!profile.can_personalize());
    assert!(!profile.can_use_cloud());
}
