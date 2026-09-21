//! Model Registry 测试。

use aicore_ai::{LocalFeatureProvider, ModelEntry, ModelRegistry};
use aicore_contracts::{
    ai::model_metadata::{ModelMetadata, ModelRuntime, PrivacyClass},
    traits::ai_provider::IAiProvider,
};

fn make_metadata(name: &str, privacy: PrivacyClass, caps: Vec<&str>) -> ModelMetadata {
    ModelMetadata {
        name: name.to_string(),
        version: "1.0.0".to_string(),
        runtime: ModelRuntime::NativeRust,
        capabilities: caps.into_iter().map(|s| s.to_string()).collect(),
        latency_budget_ms: 100,
        memory_budget_mb: 16,
        privacy_class: privacy,
        description: format!("{} model", name),
    }
}

#[test]
fn registry_starts_empty() {
    let r = ModelRegistry::new();
    assert_eq!(r.count().unwrap(), 0);
}

#[test]
fn register_and_get() {
    let r = ModelRegistry::new();
    let entry = ModelEntry::new(
        make_metadata("m1", PrivacyClass::LocalOnly, vec!["policy.recommend"]),
        "provider1",
        true,
    );
    r.register(entry.clone()).unwrap();
    assert_eq!(r.count().unwrap(), 1);

    let got = r.get("m1").unwrap().unwrap();
    assert_eq!(got, entry);
}

#[test]
fn unregister_removes() {
    let r = ModelRegistry::new();
    r.register(ModelEntry::new(
        make_metadata("m1", PrivacyClass::LocalOnly, vec![]),
        "p",
        true,
    ))
    .unwrap();

    assert!(r.unregister("m1").unwrap());
    assert!(!r.unregister("m1").unwrap());
}

#[test]
fn register_rejects_invalid_metadata() {
    let r = ModelRegistry::new();
    let mut meta = make_metadata("bad", PrivacyClass::LocalOnly, vec![]);
    meta.latency_budget_ms = 0;
    let entry = ModelEntry::new(meta, "p", true);
    assert!(r.register(entry).is_err());
}

#[test]
fn list_by_privacy() {
    let r = ModelRegistry::new();
    r.register(ModelEntry::new(
        make_metadata("local", PrivacyClass::LocalOnly, vec![]),
        "p",
        true,
    ))
    .unwrap();
    r.register(ModelEntry::new(
        make_metadata("pref", PrivacyClass::LocalPreferred, vec![]),
        "p",
        true,
    ))
    .unwrap();

    let local = r.list_by_privacy(PrivacyClass::LocalOnly).unwrap();
    assert_eq!(local.len(), 1);
    assert_eq!(local[0].metadata.name, "local");
}

#[test]
fn list_by_capability() {
    let r = ModelRegistry::new();
    r.register(ModelEntry::new(
        make_metadata("a", PrivacyClass::LocalOnly, vec!["policy.recommend"]),
        "p",
        true,
    ))
    .unwrap();
    r.register(ModelEntry::new(
        make_metadata("b", PrivacyClass::LocalOnly, vec!["workload.classify"]),
        "p",
        true,
    ))
    .unwrap();

    let cap = r.list_by_capability("policy.recommend").unwrap();
    assert_eq!(cap.len(), 1);
    assert_eq!(cap[0].metadata.name, "a");
}

#[test]
fn register_local_feature_provider() {
    let p = LocalFeatureProvider::with_default_weights().unwrap();
    let meta = p.metadata();
    let r = ModelRegistry::new();
    r.register(ModelEntry::new(meta, p.name(), p.is_available()))
        .unwrap();

    let all = r.list().unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].metadata.runtime, ModelRuntime::NativeRust);
    assert!(all[0].metadata.is_local_only());
}
