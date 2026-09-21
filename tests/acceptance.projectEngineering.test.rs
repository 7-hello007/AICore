use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}
fn exists(path: &str) -> bool {
    repo_root().join(path).exists()
}
fn read(path: &str) -> String {
    std::fs::read_to_string(repo_root().join(path)).expect(path)
}

#[test]
fn global_docs_are_present() {
    for f in [
        "docs/README.md",
        "docs/DOCUMENTATION_INDEX.md",
        "docs/SYSTEM_ARCHITECTURE.md",
        "docs/SYSTEM_REQUIREMENTS.md",
        "docs/SYSTEM_BOUNDARY.md",
        "docs/contracts/CONTRACT_ARCHITECTURE.md",
        "docs/quality/TESTING_STRATEGY.md",
        "docs/quality/TRACEABILITY.md",
    ] {
        assert!(exists(f), "missing global design document: {}", f);
    }
}

#[test]
fn versioned_destination_is_not_part_of_product_contract() {
    assert!(!exists("destination"));
    assert!(!exists("docs/v3"));
    assert!(!exists("docs/v3.2"));
    assert!(!exists("docs/v4"));
}

#[test]
fn architecture_keeps_three_tiers_independent() {
    let tier = read("docs/context/TIER_ARCHITECTURE.md");
    for term in ["L0", "L1", "L2", "independent", "Core Contracts"] {
        assert!(
            tier.to_lowercase().contains(&term.to_lowercase()),
            "tier architecture missing {}",
            term
        );
    }
}

#[test]
fn ai_control_chain_is_documented() {
    let arch = read("docs/architecture/SYSTEM_ARCHITECTURE.md");
    for term in [
        "Observe",
        "Understand",
        "Predict",
        "Decide",
        "Validate",
        "Execute",
    ] {
        assert!(arch.contains(term), "system architecture missing {}", term);
    }
}

#[test]
fn runtime_model_exists() {
    assert!(exists("models/policy_classifier_v1.json"));
}

#[test]
fn dashboard_assets_exist() {
    for f in [
        "apps/aicore-dashboard-web/static/index.html",
        "apps/aicore-dashboard-web/static/style.css",
        "apps/aicore-dashboard-web/static/app.js",
    ] {
        assert!(exists(f), "missing web dashboard asset: {}", f);
    }
}

#[test]
fn no_legacy_versioned_verifier_scripts() {
    assert!(!exists("scripts/v3"));
    assert!(!exists("scripts/v3.2"));
    assert!(exists("scripts/verify/verify_project.sh"));
}

#[test]
fn deploy_has_both_primary_platform_paths() {
    for f in [
        "deploy/linux/aicored.service",
        "deploy/linux/install.sh",
        "deploy/windows/service-install.ps1",
        "deploy/windows/service-upgrade.ps1",
    ] {
        assert!(exists(f), "missing deployment asset: {}", f);
    }
}
