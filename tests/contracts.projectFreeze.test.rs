//! V3.2 Case 1 contractually freezes acceptance testing.
//!
//! No logic is implemented, only the existence of the contract**, the integrity of the field, semantic consistency**.

use aicore_contracts::{
    ai::runtime::{
        InferenceRequest, ModelKind, ModelSource, ModelSpec, OnnxModelSpec, TensorDType,
        TensorSchema,
    },
    dashboard::snapshot::{DashboardSnapshot, TierStateEntry},
    hardware::capability::{HardwareCapability, HardwareCapabilitySet},
    platform::service::{ServiceConfig, ServiceState},
    plugin::host::{PluginHostState, PluginProcessSpec},
};

#[test]
fn real_runtime_model_kind_has_three_variants() {
    let kinds = [
        ModelKind::FeatureWeighted,
        ModelKind::Onnx,
        ModelKind::LlamaCpp,
    ];
    assert_eq!(kinds.len(), 3);
}

#[test]
fn tensor_schema_rank_and_count() {
    let s = TensorSchema {
        name: "input".to_string(),
        shape: vec![1, 4],
        dtype: TensorDType::F32,
    };
    assert_eq!(s.rank(), 2);
    assert_eq!(s.element_count(), Some(4));

    let dyn_shape = TensorSchema {
        name: "dyn".to_string(),
        shape: vec![-1, 4],
        dtype: TensorDType::F32,
    };
    assert_eq!(dyn_shape.rank(), 2);
    assert_eq!(dyn_shape.element_count(), None);
}

#[test]
fn model_spec_validates_kind_and_spec() {
    let onnx_without_spec = ModelSpec {
        name: "m".to_string(),
        version: "1.0".to_string(),
        kind: ModelKind::Onnx,
        source: ModelSource::File {
            path: "/x.onnx".to_string(),
        },
        onnx_spec: None,
    };
    assert!(onnx_without_spec.validate().is_err());

    let onnx_with_spec = ModelSpec {
        onnx_spec: Some(OnnxModelSpec {
            input_schema: vec![TensorSchema {
                name: "input".to_string(),
                shape: vec![1, 4],
                dtype: TensorDType::F32,
            }],
            output_schema: vec![],
            preferred_provider: None,
        }),
        ..onnx_without_spec
    };
    assert!(onnx_with_spec.validate().is_ok());

    let feature_with_spec = ModelSpec {
        kind: ModelKind::FeatureWeighted,
        onnx_spec: Some(OnnxModelSpec {
            input_schema: vec![],
            output_schema: vec![],
            preferred_provider: None,
        }),
        ..onnx_with_spec
    };
    assert!(feature_with_spec.validate().is_err());
}

#[test]
fn inference_request_supports_tensors() {
    let req = InferenceRequest {
        features: vec![0.1, 0.2],
        tensors: vec![],
    };
    assert_eq!(req.features.len(), 2);
    assert!(req.tensors.is_empty());
}

#[test]
fn hardware_capability_telemetry_vs_control() {
    assert!(HardwareCapability::DeviceIdentity.is_telemetry());
    assert!(HardwareCapability::UtilizationTelemetry.is_telemetry());
    assert!(!HardwareCapability::WorkloadHint.is_telemetry());
    assert!(HardwareCapability::WorkloadHint.is_control());
    assert!(HardwareCapability::ProviderSelection.is_control());
}

#[test]
fn hardware_capability_set_filters() {
    use aicore_contracts::hardware::capability::HardwareCapabilityEntry;

    let set = HardwareCapabilitySet::new("Test GPU")
        .with(HardwareCapabilityEntry::supported(
            HardwareCapability::DeviceIdentity,
        ))
        .with(HardwareCapabilityEntry::supported(
            HardwareCapability::UtilizationTelemetry,
        ))
        .with(HardwareCapabilityEntry::unsupported(
            HardwareCapability::VendorControl,
            "not implemented",
        ));

    assert!(set.is_supported(HardwareCapability::DeviceIdentity));
    assert!(!set.is_supported(HardwareCapability::VendorControl));
    assert_eq!(set.supported_telemetry().len(), 2);
    assert!(set.supported_controls().is_empty());
}

#[test]
fn dashboard_snapshot_has_schema_version() {
    let snap = DashboardSnapshot::new(1_700_000_000_000);
    assert_eq!(snap.schema_version.to_string(), "1.0");
    assert_eq!(snap.fetched_at_unix_ms, 1_700_000_000_000);
}

#[test]
fn dashboard_snapshot_default_is_empty() {
    let snap = DashboardSnapshot::default();
    assert!(snap.daemon_status.is_none());
    assert!(snap.current_context.is_none());
    assert!(snap.current_policy.is_none());
    assert!(snap.runtime_route.is_none());
    assert!(snap.tier_states.is_empty());
    assert!(snap.plugin_states.is_empty());
    assert!(snap.retention_status.is_none());
    assert!(snap.ai_runtime_health.is_none());
    assert!(snap.errors.is_empty());
}

#[test]
fn dashboard_snapshot_serde_roundtrip() {
    let snap = DashboardSnapshot::new(1_700_000_000_000);
    let json = serde_json::to_string(&snap).expect("serialize");
    let back: DashboardSnapshot = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, snap);
}

#[test]
fn dashboard_tier_state_entry() {
    let e = TierStateEntry {
        id: "l0.legacy".to_string(),
        level: "L0".to_string(),
        state: "Active".to_string(),
        capabilities: vec!["workload_observation".to_string()],
    };
    let json = serde_json::to_string(&e).unwrap();
    let back: TierStateEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(back, e);
}

#[test]
fn service_state_semantics() {
    assert!(ServiceState::Running.is_operational());
    assert!(!ServiceState::Stopped.is_operational());
    assert!(!ServiceState::Failed.is_operational());
}

#[test]
fn service_config_validate() {
    let mut c = ServiceConfig::default();
    assert!(c.validate().is_err(), "empty binary_path should be invalid");

    c.binary_path = "/usr/local/bin/aicored".to_string();
    assert!(c.validate().is_ok());

    c.name = "".to_string();
    assert!(c.validate().is_err());
}

#[test]
fn plugin_host_state_semantics() {
    assert!(PluginHostState::Running.is_operational());
    assert!(PluginHostState::Stopped.is_terminal());
    assert!(PluginHostState::Crashed.is_terminal());
    assert!(!PluginHostState::Running.is_terminal());
}

#[test]
fn plugin_process_spec_validate() {
    let spec = PluginProcessSpec {
        plugin_id: "p1".to_string(),
        artifact_path: "/tmp/p1.so".to_string(),
        timeout_ms: 1000,
        memory_limit_mb: 64,
    };
    assert!(spec.validate().is_ok());

    let bad = PluginProcessSpec {
        plugin_id: "".to_string(),
        ..spec.clone()
    };
    assert!(bad.validate().is_err());

    let no_timeout = PluginProcessSpec {
        timeout_ms: 0,
        ..spec
    };
    assert!(no_timeout.validate().is_err());
}
