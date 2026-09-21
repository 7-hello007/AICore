use aicore_contracts::{
    policy::{
        priority::PolicyPriority,
        resource_policy::{
            BackgroundPolicy, CpuPolicy, GpuPolicy, MemoryPolicy, NpuPolicy, PowerMode,
            PowerPolicy, ResourcePolicy,
        },
    },
    traits::policy_validator::IPolicyValidator,
};
use aicore_core::policy::DefaultPolicyValidator;

fn base() -> ResourcePolicy {
    ResourcePolicy {
        policy_id: "policy-1".to_string(),
        priority: PolicyPriority::High,
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
    }
}

#[test]
fn accepts_valid_policy() {
    let v = DefaultPolicyValidator::new();
    let r = v.validate(&base());
    assert!(r.accepted, "unexpected issues: {:?}", r.issues);
}

#[test]
fn rejects_empty_policy_id() {
    let v = DefaultPolicyValidator::new();
    let mut p = base();
    p.policy_id = "".to_string();
    let r = v.validate(&p);
    assert!(!r.accepted);
    assert!(r.issues.iter().any(|i| i.code == "ERR_POLICY_INVALID"));
}

#[test]
fn rejects_subresource_priority_exceeding_overall() {
    let v = DefaultPolicyValidator::new();
    let mut p = base();
    p.priority = PolicyPriority::Normal;
    p.gpu.priority = PolicyPriority::High;
    let r = v.validate(&p);
    assert!(!r.accepted);
    assert!(r.issues.iter().any(|i| i.code == "ERR_POLICY_CONFLICT"));
}

#[test]
fn rejects_invalid_background_max_cpu() {
    let v = DefaultPolicyValidator::new();
    let mut p = base();
    p.background.max_cpu_percent = Some(150.0);
    let r = v.validate(&p);
    assert!(!r.accepted);
    assert!(r.issues.iter().any(|i| i.code == "ERR_POLICY_INVALID"));
}

#[test]
fn rejects_invalid_memory_max_usage() {
    let v = DefaultPolicyValidator::new();
    let mut p = base();
    p.memory.max_usage_percent = Some(-10.0);
    let r = v.validate(&p);
    assert!(!r.accepted);
}

#[test]
fn rejects_npu_preferred_with_background_priority() {
    let v = DefaultPolicyValidator::new();
    let mut p = base();
    p.priority = PolicyPriority::High;
    p.npu.priority = PolicyPriority::Background;
    p.npu.preferred = true;
    let r = v.validate(&p);
    assert!(!r.accepted);
    assert!(r.issues.iter().any(|i| i.code == "ERR_POLICY_CONFLICT"));
}

#[test]
fn rejects_empty_affinity_cores() {
    let v = DefaultPolicyValidator::new();
    let mut p = base();
    p.cpu.affinity_cores = Some(vec![]);
    let r = v.validate(&p);
    assert!(!r.accepted);
}

#[test]
fn reports_name() {
    assert_eq!(
        DefaultPolicyValidator::new().name(),
        "default_policy_validator"
    );
}
