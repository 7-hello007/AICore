use std::sync::Arc;

use aicore_api::{ApplicationRegistry, PublicApi};
use aicore_contracts::{
    application::id::ApplicationId,
    context::workload::WorkloadType,
    policy::priority::PolicyPriority,
    task::{
        contract::TaskContract,
        preferences::{
            HardwareRequirement, LatencySensitivity, PowerPreference, PrivacyRequirement,
        },
    },
};

fn setup() -> (Arc<ApplicationRegistry>, PublicApi, ApplicationId) {
    let reg = Arc::new(ApplicationRegistry::new());
    let app = ApplicationId::new("com.example.vscode");
    reg.register(app.clone(), "VSCode", "1.0.0", vec![])
        .unwrap();
    let api = PublicApi::new(reg.clone());
    (reg, api, app)
}

#[test]
fn submit_minimal_contract_succeeds() {
    let (_reg, api, app) = setup();
    let contract = TaskContract::minimal(app);
    let result = api.submit_task(&contract).unwrap();
    assert!(result.accepted);
    assert!(result.task_id.is_none());
}

#[test]
fn submit_full_contract_succeeds() {
    let (_reg, api, app) = setup();
    let contract = TaskContract {
        application_id: app,
        task_id: Some("task-001".to_string()),
        task: Some(WorkloadType::AiInference),
        priority: Some(PolicyPriority::High),
        latency: Some(LatencySensitivity::High),
        power: Some(PowerPreference::Balanced),
        privacy: Some(PrivacyRequirement::LocalOnly),
        gpu_requirement: Some(HardwareRequirement::Optional),
        npu_requirement: Some(HardwareRequirement::Preferred),
        deadline_unix_secs: Some(1_700_000_000),
    };
    let result = api.submit_task(&contract).unwrap();
    assert!(result.accepted);
    assert_eq!(result.task_id.as_deref(), Some("task-001"));
}

#[test]
fn submit_from_unregistered_app_is_rejected() {
    let reg = Arc::new(ApplicationRegistry::new());
    let api = PublicApi::new(reg);
    let contract = TaskContract::minimal(ApplicationId::new("com.example.unknown"));
    let err = api.submit_task(&contract).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("ERR_APP_NOT_REGISTERED"));
}

#[test]
fn submit_rejects_empty_task_id() {
    let (_reg, api, app) = setup();
    let mut contract = TaskContract::minimal(app);
    contract.task_id = Some("".to_string());
    let err = api.submit_task(&contract).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("ERR_CONTRACT_INVALID"));
}

#[test]
fn submit_rejects_zero_deadline() {
    let (_reg, api, app) = setup();
    let mut contract = TaskContract::minimal(app);
    contract.deadline_unix_secs = Some(0);
    let err = api.submit_task(&contract).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("ERR_CONTRACT_INVALID"));
}

#[test]
fn contract_serializes_to_json() {
    let app = ApplicationId::new("com.example.test");
    let contract = TaskContract {
        application_id: app,
        task_id: Some("t1".to_string()),
        task: Some(WorkloadType::Development),
        latency: Some(LatencySensitivity::High),
        ..TaskContract::minimal(ApplicationId::new("com.example.test"))
    };
    let json = serde_json::to_string(&contract).unwrap();
    assert!(json.contains("development"));
    assert!(json.contains("high"));

    let back: TaskContract = serde_json::from_str(&json).unwrap();
    assert_eq!(contract, back);
}

#[test]
fn api_can_unregister_app() {
    let (reg, api, app) = setup();
    assert!(reg.is_registered(&app).unwrap());
    api.unregister(&app).unwrap();
    assert!(!reg.is_registered(&app).unwrap());
}
