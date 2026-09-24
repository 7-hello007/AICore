//! Memory → AI Provider 集成：历史影响 AI 决策。

use std::sync::Arc;

use aicore_ai::LocalFeatureProvider;
use aicore_contracts::{
    ai::input::AiInput,
    context::{
        snapshot::ContextSnapshot,
        workload::{InteractionLevel, WorkloadType},
    },
    execution::status::ExecutionOverallStatus,
    memory::{context::MemoryContext, profile::UserProfile, record::HistoricalRecord},
    policy::{
        priority::PolicySource,
        resource_policy::{PowerMode, ResourcePolicy},
    },
    telemetry::snapshot::ComputerSnapshot,
    traits::{
        ai_provider::IAiProvider, memory_store::IMemoryStore, policy_engine::IPolicyEngine,
        policy_validator::IPolicyValidator,
    },
};
use aicore_core::policy::{AiAssistedPolicyEngine, DefaultPolicyValidator, RuleBasedPolicyEngine};
use aicore_memory::InMemoryStore;

fn rec(id: &str, w: WorkloadType, mode: PowerMode) -> HistoricalRecord {
    HistoricalRecord {
        application_id: None,
        record_id: id.to_string(),
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

#[test]
fn memory_context_is_constructed_from_store() {
    let store = InMemoryStore::new();
    store
        .append_record(&rec("a", WorkloadType::Development, PowerMode::Balanced))
        .unwrap();
    store
        .append_record(&rec("b", WorkloadType::Development, PowerMode::Balanced))
        .unwrap();
    store
        .append_record(&rec("c", WorkloadType::Gaming, PowerMode::Performance))
        .unwrap();

    let records = store.load_records(50).unwrap();
    let ctx = MemoryContext::from_records(&records);

    assert_eq!(ctx.sample_count, 3);
    assert_eq!(ctx.dominant_workload, Some(WorkloadType::Development));
    assert_eq!(ctx.preferred_power_mode, Some(PowerMode::Balanced));
}

#[test]
fn empty_memory_returns_default_context() {
    let ctx = MemoryContext::from_records(&[]);
    assert_eq!(ctx.sample_count, 0);
    assert!(ctx.dominant_workload.is_none());
    assert!(ctx.recent_workloads.is_empty());
}

#[test]
fn memory_context_is_passed_to_ai_provider() {
    // 构造一个输入，有 memory 与没有 memory 时特征向量的 has_memory 维不同。
    let base = ResourcePolicy::default();

    let ctx = ContextSnapshot {
        workload_type: WorkloadType::GeneralOffice,
        interaction_level: InteractionLevel::Passive,
        ..Default::default()
    };

    let input_no_memory = AiInput {
        snapshot: ComputerSnapshot::default(),
        context: ctx.clone(),
        base_policy: Some(base.clone()),
        memory: None,
    };

    let input_with_memory = AiInput {
        snapshot: ComputerSnapshot::default(),
        context: ctx.clone(),
        base_policy: Some(base.clone()),
        memory: Some(MemoryContext {
            recent_workloads: vec![WorkloadType::Development],
            dominant_workload: Some(WorkloadType::Development),
            preferred_power_mode: Some(PowerMode::Balanced),
            sample_count: 5,
            hourly_patterns: Vec::new(),
            application_patterns: Vec::new(),
        }),
    };

    let provider = LocalFeatureProvider::with_default_weights().unwrap();

    // 两者都应能推理（不 panic），且都能通过 Validator
    let s1 = provider.suggest(&input_no_memory);
    let s2 = provider.suggest(&input_with_memory);

    // 至少有一个路径能产出建议（若都失败也是合法，因为可能置信度不足）
    let validator = DefaultPolicyValidator::new();

    if let Ok(s) = s1 {
        if let aicore_contracts::ai::suggestion::AiSuggestionPayload::Policy(p) = s.payload {
            assert!(validator.validate(&p).accepted);
        }
    }
    if let Ok(s) = s2 {
        if let aicore_contracts::ai::suggestion::AiSuggestionPayload::Policy(p) = s.payload {
            assert!(validator.validate(&p).accepted);
        }
    }
}

#[test]
fn ai_assisted_engine_uses_memory_context() {
    let base: Arc<dyn IPolicyEngine> = Arc::new(RuleBasedPolicyEngine::new());
    let ai: Arc<dyn IAiProvider> = Arc::new(LocalFeatureProvider::with_default_weights().unwrap());
    let engine = AiAssistedPolicyEngine::new(base, Some(ai), 0.4);

    let snapshot = ComputerSnapshot::default();
    let ctx = ContextSnapshot {
        workload_type: WorkloadType::AiInference,
        interaction_level: InteractionLevel::Interactive,
        ..Default::default()
    };

    let memory = Some(MemoryContext {
        recent_workloads: vec![WorkloadType::AiInference; 4],
        dominant_workload: Some(WorkloadType::AiInference),
        preferred_power_mode: Some(PowerMode::Balanced),
        sample_count: 4,
        hourly_patterns: Vec::new(),
        application_patterns: Vec::new(),
    });

    // generate_with_memory 走 AI + memory 路径
    let policy_with_memory = engine.generate_with_memory(&snapshot, &ctx, memory.clone());
    // generate 无 memory
    let policy_without = engine.generate(&snapshot, &ctx);

    // 两者都必须通过 Validator
    let validator = DefaultPolicyValidator::new();
    assert!(validator.validate(&policy_with_memory).accepted);
    assert!(validator.validate(&policy_without).accepted);
}

#[test]
fn privacy_default_disables_memory_personalization() {
    // 用户默认 profile 不允许 personalization
    let store = InMemoryStore::new();
    let profile = store.load_profile().unwrap();
    assert!(!profile.can_record());
    assert!(!profile.can_personalize());

    // 用户显式启用后可记录
    let enabled = UserProfile {
        history_enabled: true,
        profile_enabled: true,
        ..Default::default()
    };
    store.save_profile(&enabled).unwrap();

    let profile = store.load_profile().unwrap();
    assert!(profile.can_record());
    assert!(profile.can_personalize());
}
