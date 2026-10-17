//! Dynamic load example cdylib plugin.
//!
//! Prefix: `cargo build --workspace` (build plugin-hello).
//! If .so does not exist, test automatically skip and print the hint.

use std::path::PathBuf;

use aicore_plugin::DynamicPluginLoader;

fn example_plugin_path() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().unwrap();
    let target_dir = workspace_root.join("target").join("debug");

    #[cfg(target_os = "linux")]
    let name = "libplugin_hello.so";
    #[cfg(target_os = "windows")]
    let name = "plugin_hello.dll";
    #[cfg(target_os = "macos")]
    let name = "libplugin_hello.dylib";

    target_dir.join(name)
}

#[test]
fn load_example_plugin_and_execute() {
    let path = example_plugin_path();
    if !path.exists() {
        eprintln!(
            "skip: example plugin not built; run `cargo build -p plugin-hello` first ({})",
            path.display()
        );
        return;
    }

    // This locally built fixture is explicitly trusted by the test operator.
    let pin = aicore_plugin::SignatureVerifier::compute_sha256(&path).unwrap();
    let loader = DynamicPluginLoader::new().with_expected_signature(pin);
    let plugin = loader.load(&path).expect("load");

    assert_eq!(plugin.name, "plugin-hello");
    assert_eq!(plugin.api_version, 1);
    assert!(plugin.signature.is_some());

    // Implementation: input* 2 + 1
    assert_eq!(plugin.execute(1), 3);
    assert_eq!(plugin.execute(10), 21);
    assert_eq!(plugin.execute(0), 1);
}

#[test]
fn load_example_plugin_with_signature() {
    let path = example_plugin_path();
    if !path.exists() {
        eprintln!("skip: example plugin not built");
        return;
    }

    let sig = aicore_plugin::SignatureVerifier::compute_sha256(&path).unwrap();
    let loader = DynamicPluginLoader::new().with_expected_signature(sig);
    let plugin = loader.load(&path).expect("load with sig");
    assert_eq!(plugin.name, "plugin-hello");
}

#[test]
fn load_example_plugin_with_wrong_signature_fails() {
    let path = example_plugin_path();
    if !path.exists() {
        eprintln!("skip: example plugin not built");
        return;
    }

    let loader = DynamicPluginLoader::new().with_expected_signature(
        aicore_contracts::plugin::signature::PluginSignature::sha256(
            "0000000000000000000000000000000000000000000000000000000000000000",
        ),
    );
    assert!(loader.load(&path).is_err());
}
