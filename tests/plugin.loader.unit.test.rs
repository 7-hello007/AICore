//! DynamicPluginLoader 错误路径测试。

use std::path::PathBuf;

use aicore_plugin::{DynamicPluginLoader, LoadError};

#[test]
fn load_missing_file_fails() {
    let loader = DynamicPluginLoader::new();
    let err = loader.load("/nonexistent/plugin.so").unwrap_err();
    assert!(matches!(err, LoadError::NotFound(_)));
}

#[test]
fn load_invalid_file_fails() {
    let dir = std::env::temp_dir().join(format!("aicore-plugin-loader-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("not-a-library.bin");
    std::fs::write(&path, b"this is not an ELF/DLL").unwrap();

    let loader = DynamicPluginLoader::new();
    let err = loader.load(&path).unwrap_err();
    // 报 Open 错误，不是 panic
    assert!(matches!(err, LoadError::Open(_)));

    let _ = std::fs::remove_file(&path);
}

#[test]
fn signature_mismatch_fails_before_open() {
    use aicore_contracts::plugin::signature::PluginSignature;

    let dir = std::env::temp_dir().join(format!("aicore-plugin-sig-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("fake.so");
    std::fs::write(&path, b"data").unwrap();

    let loader = DynamicPluginLoader::new().with_expected_signature(PluginSignature::sha256(
        "0000000000000000000000000000000000000000000000000000000000000000",
    ));
    let err = loader.load(&path).unwrap_err();
    assert!(matches!(err, LoadError::Signature(_)));

    let _ = std::fs::remove_file(&path);
}

#[test]
fn loader_metadata() {
    let loader = DynamicPluginLoader::new();
    assert!(loader.expected_signature().is_none());
}

fn _unused(_p: PathBuf) {}
