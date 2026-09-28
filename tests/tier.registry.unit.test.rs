//! TierRegistry test with TierRegistration.

use aicore_contracts::errors::AicoreError;
use aicore_contracts::tier::{
    capability::TierCapability, id::TierId, level::TierLevel, registration::TierRegistration,
};
use aicore_tier::{RegistryError, TierRegistry};

fn l0() -> TierRegistration {
    TierRegistration::new(TierId::new("l0.legacy"), TierLevel::L0, "1.0.0")
        .with_capability(TierCapability::WorkloadObservation)
        .with_capability(TierCapability::TransparentAdaptation)
        .with_description("L0 legacy adapter")
}

fn l1() -> TierRegistration {
    TierRegistration::new(TierId::new("l1.aware"), TierLevel::L1, "1.0.0")
        .with_capability(TierCapability::TaskMetadata)
        .with_capability(TierCapability::ResourceHints)
}

fn l2() -> TierRegistration {
    TierRegistration::new(TierId::new("l2.native"), TierLevel::L2, "1.0.0")
        .with_capability(TierCapability::NativeTaskControl)
        .with_capability(TierCapability::UserIntent)
}

#[test]
fn register_single_tier() {
    let r = TierRegistry::new();
    r.register(l0()).unwrap();
    assert_eq!(r.count().unwrap(), 1);
    assert!(!r.is_empty().unwrap());
}

#[test]
fn duplicate_id_rejected() {
    let r = TierRegistry::new();
    r.register(l0()).unwrap();
    assert!(matches!(
        r.register(l0()),
        Err(RegistryError::DuplicateId(_))
    ));
}

#[test]
fn empty_version_rejected() {
    let r = TierRegistry::new();
    let reg = TierRegistration::new(TierId::new("x"), TierLevel::L0, "");
    assert!(matches!(
        r.register(reg),
        Err(RegistryError::InvalidRegistration(
            AicoreError::TierInvalid(_)
        ))
    ));
}

#[test]
fn no_capabilities_rejected() {
    let r = TierRegistry::new();
    let reg = TierRegistration::new(TierId::new("x"), TierLevel::L0, "1.0.0");
    assert!(matches!(
        r.register(reg),
        Err(RegistryError::InvalidRegistration(
            AicoreError::TierInvalid(_)
        ))
    ));
}

#[test]
fn l0_cannot_declare_native_control() {
    let r = TierRegistry::new();
    let reg = TierRegistration::new(TierId::new("bad"), TierLevel::L0, "1.0.0")
        .with_capability(TierCapability::NativeTaskControl);
    assert!(matches!(
        r.register(reg),
        Err(RegistryError::InvalidRegistration(
            AicoreError::TierLevelCannotControlNative(TierLevel::L0)
        ))
    ));
}

#[test]
fn invalid_id_rejected() {
    let r = TierRegistry::new();
    let reg = TierRegistration::new(TierId::new("has space"), TierLevel::L0, "1.0.0")
        .with_capability(TierCapability::WorkloadObservation);
    assert!(matches!(
        r.register(reg),
        Err(RegistryError::InvalidRegistration(
            AicoreError::TierInvalid(_)
        ))
    ));
}

#[test]
fn find_by_capability() {
    let r = TierRegistry::new();
    r.register(l0()).unwrap();
    r.register(l1()).unwrap();
    r.register(l2()).unwrap();

    let obs = r
        .find_by_capability(TierCapability::WorkloadObservation)
        .unwrap();
    assert_eq!(obs.len(), 1);
    assert_eq!(obs[0].id.as_str(), "l0.legacy");

    let native = r
        .find_by_capability(TierCapability::NativeTaskControl)
        .unwrap();
    assert_eq!(native.len(), 1);
    assert_eq!(native[0].id.as_str(), "l2.native");
}

#[test]
fn find_by_level() {
    let r = TierRegistry::new();
    r.register(l0()).unwrap();
    r.register(l1()).unwrap();
    r.register(l2()).unwrap();

    assert_eq!(r.find_by_level(TierLevel::L0).unwrap().len(), 1);
    assert_eq!(r.find_by_level(TierLevel::L1).unwrap().len(), 1);
    assert_eq!(r.find_by_level(TierLevel::L2).unwrap().len(), 1);
}

#[test]
fn unregister_removes() {
    let r = TierRegistry::new();
    r.register(l0()).unwrap();
    let id = TierId::new("l0.legacy");
    assert!(r.unregister(&id).unwrap());
    assert!(!r.unregister(&id).unwrap());
    assert!(r.is_empty().unwrap());
}

#[test]
fn list_is_sorted_by_id() {
    let r = TierRegistry::new();
    r.register(l2()).unwrap();
    r.register(l0()).unwrap();
    r.register(l1()).unwrap();

    let list = r.list().unwrap();
    assert_eq!(list[0].id.as_str(), "l0.legacy");
    assert_eq!(list[1].id.as_str(), "l1.aware");
    assert_eq!(list[2].id.as_str(), "l2.native");
}

#[test]
fn empty_registry_is_valid() {
    let r = TierRegistry::new();
    assert!(r.is_empty().unwrap());
    assert_eq!(r.count().unwrap(), 0);
    assert!(r.list().unwrap().is_empty());
}
