use aicore_contracts::{
    execution::{status::ExecutionOverallStatus, targets::ExecutionTargets},
    policy::resource_policy::ResourcePolicy,
    traits::{
        policy_executor::IPolicyExecutor,
        post_condition_checker::{IPostConditionChecker, PostConditionResult},
    },
};

use aicore_core::model::{
    ModelEnvironment, ModelPolicyExecutor, ModelPostConditionChecker, ModelTargetProvider,
    MODEL_BACKGROUND_PID, MODEL_FOREGROUND_PID,
};

#[test]
fn model_target_provider_returns_virtual_targets() {
    let provider = ModelTargetProvider::default();
    let targets = provider.execution_targets();

    assert_eq!(targets.foreground_pids, vec![MODEL_FOREGROUND_PID]);

    assert_eq!(targets.background_pids, vec![MODEL_BACKGROUND_PID]);
}

#[test]
fn model_executor_produces_real_success_without_host_execution() {
    let environment = ModelEnvironment::new();

    let executor = ModelPolicyExecutor::new(environment.clone());

    let policy = ResourcePolicy {
        policy_id: "model-test-1".to_string(),
        ..Default::default()
    };

    let targets = ModelTargetProvider::default().execution_targets();

    let result = executor.execute(&policy, &targets);

    assert_eq!(result.overall_status, ExecutionOverallStatus::AllSucceeded);

    assert!(!result.outcomes.is_empty());

    assert!(result.outcomes.iter().all(|outcome| {
        matches!(
            outcome.status,
            aicore_contracts::execution::status::ExecutionStatus::Success
        )
    }));

    let state = environment.snapshot();

    assert_eq!(state.last_policy_id.as_deref(), Some("model-test-1"));

    assert_eq!(
        state.processes.get(&MODEL_FOREGROUND_PID).unwrap().priority,
        policy.cpu.priority
    );
}

#[test]
fn empty_targets_are_replaced_by_model_targets() {
    let environment = ModelEnvironment::new();

    let executor = ModelPolicyExecutor::new(environment.clone());

    let policy = ResourcePolicy {
        policy_id: "model-empty-target-test".to_string(),
        ..Default::default()
    };
    let result = executor.execute(&policy, &ExecutionTargets::default());

    assert_eq!(result.overall_status, ExecutionOverallStatus::AllSucceeded);

    assert!(result
        .outcomes
        .iter()
        .any(|outcome| outcome.resource == "model.process.priority"));
}

#[test]
fn model_post_condition_passes_after_apply() {
    let environment = ModelEnvironment::new();

    let executor = ModelPolicyExecutor::new(environment.clone());

    let policy = ResourcePolicy {
        policy_id: "model-verify-test".to_string(),
        ..Default::default()
    };

    let targets = ModelTargetProvider::default().execution_targets();

    let result = executor.execute(&policy, &targets);

    let checker = ModelPostConditionChecker::new(environment);

    assert!(matches!(
        checker.verify(&policy, &result),
        PostConditionResult::Passed
    ));
}
