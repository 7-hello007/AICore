use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use aicore_contracts::errors::AicoreResult;
use aicore_contracts::plugin::{
    host::{IPluginHost, PluginHostState, PluginProcessHandle, PluginProcessSpec},
    manifest::{PluginManifest, TrustLevel},
    permission::PluginPermission,
    signature::PluginSignature,
    state::PluginState,
};
use serde::{Deserialize, Serialize};

use crate::{
    discovery::DiscoveredPlugin,
    host::protocol::{HostRequest, HostResponse},
    registry::PluginRegistry,
    signature::SignatureVerifier,
    trust::{TrustEntry, TrustStore},
    SubprocessPluginHost,
};

static STAGE_SEQUENCE: AtomicU64 = AtomicU64::new(0);
const EXECUTE_INTERFACE: &str = "extension.execute";

/// Operator-controlled activation policy. A plugin manifest cannot grant
/// itself trust or extra permissions by changing its own JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginActivationPolicy {
    pub pins: Vec<PluginActivationPin>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginActivationPin {
    pub plugin_id: String,
    pub manifest_path: PathBuf,
    pub manifest_sha256: String,
    pub artifact_path: PathBuf,
    pub artifact_sha256: String,
    pub trust_level: TrustLevel,
    pub signature: Option<PluginSignature>,
    pub trusted_public_key_hex: Option<String>,
    pub allowed_interfaces: Vec<String>,
    pub allowed_permissions: Vec<PluginPermission>,
    pub allow_control: bool,
    pub timeout_ms: u64,
    pub memory_limit_mb: u64,
}

/// Extends the existing process host with its bounded request protocol.
pub trait IManagedPluginHost: IPluginHost {
    fn call(
        &self,
        handle: &PluginProcessHandle,
        request: &HostRequest,
    ) -> AicoreResult<HostResponse>;
}

impl IManagedPluginHost for SubprocessPluginHost {
    fn call(
        &self,
        handle: &PluginProcessHandle,
        request: &HostRequest,
    ) -> AicoreResult<HostResponse> {
        SubprocessPluginHost::call(self, handle, request)
    }
}

struct ActivePlugin {
    handle: PluginProcessHandle,
    stage_dir: PathBuf,
    staged_path: PathBuf,
    interfaces: Vec<String>,
}

/// Explicitly activates pinned, verified subprocess plugins. Only the
/// existing generic `extension.execute` protocol is currently bindable;
/// hardware/provider interfaces require their own typed protocol adapters.
pub struct PluginActivationManager {
    registry: Arc<PluginRegistry>,
    host: Arc<dyn IManagedPluginHost>,
    active: Mutex<HashMap<String, ActivePlugin>>,
}

impl PluginActivationManager {
    pub fn new(registry: Arc<PluginRegistry>, host: Arc<dyn IManagedPluginHost>) -> Self {
        Self {
            registry,
            host,
            active: Mutex::new(HashMap::new()),
        }
    }

    pub fn activate(
        &self,
        discovered: &DiscoveredPlugin,
        pin: &PluginActivationPin,
    ) -> Result<(), ActivationError> {
        let name = &discovered.manifest.name;
        if self
            .active
            .lock()
            .map_err(|_| ActivationError::Lifecycle("active lock poisoned".into()))?
            .contains_key(name)
        {
            return Err(ActivationError::Lifecycle(
                "plugin is already active".into(),
            ));
        }
        self.registry
            .transition(name, PluginState::Validating, None)
            .map_err(|error| ActivationError::Lifecycle(error.to_string()))?;
        let result = self.verify_and_spawn(discovered, pin);
        match result {
            Ok(active) => {
                if let Err(error) = self.registry.transition(name, PluginState::Ready, None) {
                    self.discard_unpublished(active);
                    return Err(ActivationError::Lifecycle(error.to_string()));
                }
                let mut handles = match self.active.lock() {
                    Ok(handles) => handles,
                    Err(_) => {
                        self.discard_unpublished(active);
                        let _ = self.registry.transition(
                            name,
                            PluginState::Disabled,
                            Some("active lock poisoned".into()),
                        );
                        return Err(ActivationError::Lifecycle("active lock poisoned".into()));
                    },
                };
                handles.insert(name.clone(), active);
                if let Err(error) = self.registry.transition(name, PluginState::Active, None) {
                    if let Some(active) = handles.remove(name) {
                        self.discard_unpublished(active);
                    }
                    let _ = self.registry.transition(
                        name,
                        PluginState::Disabled,
                        Some(error.to_string()),
                    );
                    return Err(ActivationError::Lifecycle(error.to_string()));
                }
                Ok(())
            },
            Err(error) => {
                self.registry
                    .transition(name, PluginState::Rejected, Some(error.to_string()))
                    .map_err(|transition| ActivationError::Lifecycle(transition.to_string()))?;
                Err(error)
            },
        }
    }

    pub fn reject_unpinned(&self, discovered: &DiscoveredPlugin) -> Result<(), ActivationError> {
        let name = &discovered.manifest.name;
        self.registry
            .transition(name, PluginState::Validating, None)
            .and_then(|_| {
                self.registry.transition(
                    name,
                    PluginState::Rejected,
                    Some("no operator-pinned activation policy".into()),
                )
            })
            .map_err(|error| ActivationError::Lifecycle(error.to_string()))
    }

    pub fn call_extension(&self, plugin_id: &str, input: u32) -> Result<u32, ActivationError> {
        let handle = {
            let active = self
                .active
                .lock()
                .map_err(|_| ActivationError::Lifecycle("active lock poisoned".into()))?;
            let entry = active
                .get(plugin_id)
                .ok_or_else(|| ActivationError::NotActive(plugin_id.into()))?;
            if !entry
                .interfaces
                .iter()
                .any(|name| name == EXECUTE_INTERFACE)
            {
                return Err(ActivationError::Rejected(
                    "extension.execute interface was not bound".into(),
                ));
            }
            entry.handle.clone()
        };
        if self.host.state(&handle) != PluginHostState::Running {
            self.mark_failed(plugin_id, "plugin process is no longer running");
            return Err(ActivationError::NotActive(plugin_id.into()));
        }
        match self.host.call(&handle, &HostRequest::Execute { input }) {
            Ok(HostResponse::Result { output }) => Ok(output),
            Ok(other) => {
                self.mark_failed(plugin_id, "plugin returned an unexpected response");
                Err(ActivationError::Rejected(format!(
                    "unexpected plugin response: {:?}",
                    other
                )))
            },
            Err(error) => {
                self.mark_failed(plugin_id, &error.to_string());
                Err(ActivationError::Host(error.to_string()))
            },
        }
    }

    pub fn refresh_health(&self) {
        let failed: Vec<String> = match self.active.lock() {
            Ok(active) => active
                .iter()
                .filter(|(_, entry)| self.host.state(&entry.handle) != PluginHostState::Running)
                .map(|(name, _)| name.clone())
                .collect(),
            Err(_) => return,
        };
        for name in failed {
            self.mark_failed(&name, "plugin process exited");
        }
    }

    pub fn stop(&self, plugin_id: &str) -> Result<(), ActivationError> {
        let active = self
            .active
            .lock()
            .map_err(|_| ActivationError::Lifecycle("active lock poisoned".into()))?
            .remove(plugin_id)
            .ok_or_else(|| ActivationError::NotActive(plugin_id.into()))?;
        let stop = self.host.stop(&active.handle);
        cleanup_stage(&active.stage_dir, &active.staged_path);
        match stop {
            Ok(()) => self
                .registry
                .transition(plugin_id, PluginState::Disabled, Some("stopped".into()))
                .map_err(|error| ActivationError::Lifecycle(error.to_string())),
            Err(error) => {
                let _ = self.registry.transition(
                    plugin_id,
                    PluginState::Failed,
                    Some(error.to_string()),
                );
                Err(ActivationError::Host(error.to_string()))
            },
        }
    }

    fn mark_failed(&self, plugin_id: &str, reason: &str) {
        if let Ok(mut active) = self.active.lock() {
            if let Some(entry) = active.remove(plugin_id) {
                let _ = self.host.stop(&entry.handle);
                cleanup_stage(&entry.stage_dir, &entry.staged_path);
            }
        }
        let _ = self
            .registry
            .transition(plugin_id, PluginState::Failed, Some(reason.to_string()));
    }

    fn discard_unpublished(&self, active: ActivePlugin) {
        let _ = self.host.stop(&active.handle);
        cleanup_stage(&active.stage_dir, &active.staged_path);
    }

    fn verify_and_spawn(
        &self,
        discovered: &DiscoveredPlugin,
        pin: &PluginActivationPin,
    ) -> Result<ActivePlugin, ActivationError> {
        let manifest = &discovered.manifest;
        if !pin.manifest_path.is_absolute() || !pin.artifact_path.is_absolute() {
            return Err(ActivationError::Rejected(
                "pinned paths must be absolute".into(),
            ));
        }
        if pin.plugin_id != manifest.name
            || pin.trust_level != manifest.trust_level
            || !matches!(pin.trust_level, TrustLevel::BuiltIn | TrustLevel::Trusted)
            || std::fs::canonicalize(&pin.manifest_path).ok()
                != std::fs::canonicalize(&discovered.path).ok()
        {
            return Err(ActivationError::Rejected(
                "manifest identity or trust level differs from pin".into(),
            ));
        }
        let manifest_bytes = std::fs::read(&pin.manifest_path)
            .map_err(|error| ActivationError::Io(error.to_string()))?;
        verify_hash(&manifest_bytes, &pin.manifest_sha256)?;
        let current_manifest: PluginManifest = serde_json::from_slice(&manifest_bytes)
            .map_err(|error| ActivationError::Rejected(error.to_string()))?;
        if current_manifest != *manifest {
            return Err(ActivationError::Rejected(
                "manifest changed after discovery".into(),
            ));
        }
        if manifest.interfaces.iter().any(|interface| {
            interface != EXECUTE_INTERFACE || !pin.allowed_interfaces.contains(interface)
        }) || manifest.interfaces.is_empty()
            || manifest
                .permissions
                .iter()
                .any(|permission| !pin.allowed_permissions.contains(permission))
            || manifest.requests_control()
        {
            return Err(ActivationError::Rejected(
                "manifest requests an unbound interface or unpinned permission".into(),
            ));
        }
        let bytes = std::fs::read(&pin.artifact_path)
            .map_err(|error| ActivationError::Io(error.to_string()))?;
        verify_hash(&bytes, &pin.artifact_sha256)?;
        let trust = TrustStore::new();
        let entry = match pin.trust_level {
            TrustLevel::BuiltIn => TrustEntry::built_in(&pin.plugin_id),
            TrustLevel::Trusted => {
                let key = pin.trusted_public_key_hex.as_ref().ok_or_else(|| {
                    ActivationError::Rejected("trusted plugin lacks a pinned key".into())
                })?;
                TrustEntry::trusted(&pin.plugin_id, key)
            },
            _ => return Err(ActivationError::Rejected("untrusted plugin".into())),
        };
        trust
            .register(TrustEntry {
                allow_control: pin.allow_control,
                ..entry
            })
            .map_err(|error| ActivationError::Rejected(error.to_string()))?;
        let verification = trust
            .verify(&pin.plugin_id, &bytes, pin.signature.as_ref())
            .map_err(|error| ActivationError::Rejected(error.to_string()))?;
        if !verification.accepted
            || !trust
                .permits_control(&pin.plugin_id, &manifest.permissions)
                .map_err(|error| ActivationError::Rejected(error.to_string()))?
        {
            return Err(ActivationError::Rejected(verification.reason));
        }

        let stage_dir = stage_verified_artifact(&pin.artifact_path, &bytes)?;
        let staged_path = stage_dir.join(
            pin.artifact_path
                .file_name()
                .ok_or_else(|| ActivationError::Rejected("artifact lacks file name".into()))?,
        );
        let spec = PluginProcessSpec {
            plugin_id: pin.plugin_id.clone(),
            artifact_path: staged_path.to_string_lossy().to_string(),
            timeout_ms: pin.timeout_ms,
            memory_limit_mb: pin.memory_limit_mb,
        };
        if let Err(error) = spec.validate() {
            cleanup_stage(&stage_dir, &staged_path);
            return Err(ActivationError::Rejected(error.to_string()));
        }
        let handle = match self.host.spawn(&spec) {
            Ok(handle) => handle,
            Err(error) => {
                cleanup_stage(&stage_dir, &staged_path);
                return Err(ActivationError::Host(error.to_string()));
            },
        };
        if !matches!(
            self.host.call(&handle, &HostRequest::Ping),
            Ok(HostResponse::Pong)
        ) {
            let _ = self.host.stop(&handle);
            cleanup_stage(&stage_dir, &staged_path);
            return Err(ActivationError::Rejected(
                "plugin health handshake failed".into(),
            ));
        }
        Ok(ActivePlugin {
            handle,
            stage_dir,
            staged_path,
            interfaces: manifest.interfaces.clone(),
        })
    }
}

impl Drop for PluginActivationManager {
    fn drop(&mut self) {
        let names: Vec<String> = self
            .active
            .lock()
            .map(|active| active.keys().cloned().collect())
            .unwrap_or_default();
        for name in names {
            let _ = self.stop(&name);
        }
    }
}

fn verify_hash(bytes: &[u8], expected: &str) -> Result<(), ActivationError> {
    if expected.len() != 64 || !expected.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ActivationError::Rejected("invalid pinned SHA-256".into()));
    }
    let actual = SignatureVerifier::compute_sha256_from_bytes(bytes);
    if !actual.hash_hex.eq_ignore_ascii_case(expected) {
        return Err(ActivationError::Rejected("pinned SHA-256 mismatch".into()));
    }
    Ok(())
}

fn stage_verified_artifact(path: &Path, bytes: &[u8]) -> Result<PathBuf, ActivationError> {
    let file_name = path
        .file_name()
        .ok_or_else(|| ActivationError::Rejected("artifact lacks file name".into()))?;
    let sequence = STAGE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "aicore-verified-plugin-{}-{}",
        std::process::id(),
        sequence
    ));
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .map_err(|error| ActivationError::Io(error.to_string()))?;
    }
    #[cfg(not(unix))]
    std::fs::create_dir(&root).map_err(|error| ActivationError::Io(error.to_string()))?;
    let file = root.join(file_name);
    if let Err(error) = std::fs::write(&file, bytes) {
        cleanup_stage(&root, &file);
        return Err(ActivationError::Io(error.to_string()));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(error) = std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o700))
        {
            cleanup_stage(&root, &file);
            return Err(ActivationError::Io(error.to_string()));
        }
    }
    Ok(root)
}

fn cleanup_stage(directory: &Path, file: &Path) {
    // Only unlink the file we created; a replaced directory must never become
    // the target of a recursive cleanup.
    let _ = std::fs::remove_file(file);
    let _ = std::fs::remove_dir(directory);
}

#[derive(Debug, thiserror::Error)]
pub enum ActivationError {
    #[error("plugin rejected: {0}")]
    Rejected(String),
    #[error("plugin host failed: {0}")]
    Host(String),
    #[error("plugin lifecycle failed: {0}")]
    Lifecycle(String),
    #[error("plugin file error: {0}")]
    Io(String),
    #[error("plugin is not active: {0}")]
    NotActive(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use aicore_contracts::plugin::manifest::PluginKind;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    #[derive(Default)]
    struct MockHost {
        spawned: AtomicUsize,
        stopped: AtomicUsize,
        crashed: AtomicBool,
    }

    impl IPluginHost for MockHost {
        fn spawn(&self, spec: &PluginProcessSpec) -> AicoreResult<PluginProcessHandle> {
            assert!(Path::new(&spec.artifact_path).exists());
            self.spawned.fetch_add(1, Ordering::SeqCst);
            Ok(PluginProcessHandle {
                handle_id: "mock-handle".into(),
                plugin_id: spec.plugin_id.clone(),
                pid: 1,
                started_at_unix_ms: 0,
            })
        }

        fn stop(&self, _handle: &PluginProcessHandle) -> AicoreResult<()> {
            self.stopped.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn state(&self, _handle: &PluginProcessHandle) -> PluginHostState {
            if self.crashed.load(Ordering::SeqCst) {
                PluginHostState::Crashed
            } else {
                PluginHostState::Running
            }
        }

        fn recent_crashes(&self) -> Vec<aicore_contracts::plugin::host::PluginCrashEvent> {
            Vec::new()
        }

        fn clear_crashes(&self) {}

        fn host_name(&self) -> &'static str {
            "mock_plugin_host"
        }
    }

    impl IManagedPluginHost for MockHost {
        fn call(
            &self,
            _handle: &PluginProcessHandle,
            request: &HostRequest,
        ) -> AicoreResult<HostResponse> {
            Ok(match request {
                HostRequest::Ping => HostResponse::Pong,
                HostRequest::Execute { input } => HostResponse::Result { output: input + 1 },
                HostRequest::Shutdown => HostResponse::ShuttingDown,
            })
        }
    }

    struct Fixture {
        root: PathBuf,
        discovered: DiscoveredPlugin,
        pin: PluginActivationPin,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "aicore-plugin-pin-test-{}-{}",
                std::process::id(),
                STAGE_SEQUENCE.fetch_add(1, Ordering::SeqCst)
            ));
            std::fs::create_dir(&root).unwrap();
            let manifest_path = root.join("echo.plugin.json");
            let artifact_path = root.join("echo.mock");
            let manifest = PluginManifest {
                name: "echo".into(),
                version: "1.0".into(),
                api_version: "1.0".into(),
                kind: PluginKind::Extension,
                trust_level: TrustLevel::BuiltIn,
                interfaces: vec![EXECUTE_INTERFACE.into()],
                permissions: vec![],
                description: String::new(),
            };
            let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
            let artifact_bytes = b"isolated-mock-artifact";
            std::fs::write(&manifest_path, &manifest_bytes).unwrap();
            std::fs::write(&artifact_path, artifact_bytes).unwrap();
            let pin = PluginActivationPin {
                plugin_id: "echo".into(),
                manifest_path: manifest_path.clone(),
                manifest_sha256: SignatureVerifier::compute_sha256_from_bytes(&manifest_bytes)
                    .hash_hex,
                artifact_path,
                artifact_sha256: SignatureVerifier::compute_sha256_from_bytes(artifact_bytes)
                    .hash_hex,
                trust_level: TrustLevel::BuiltIn,
                signature: None,
                trusted_public_key_hex: None,
                allowed_interfaces: vec![EXECUTE_INTERFACE.into()],
                allowed_permissions: vec![],
                allow_control: false,
                timeout_ms: 1000,
                memory_limit_mb: 16,
            };
            Self {
                root,
                discovered: DiscoveredPlugin {
                    path: manifest_path,
                    manifest,
                },
                pin,
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.pin.manifest_path);
            let _ = std::fs::remove_file(&self.pin.artifact_path);
            let _ = std::fs::remove_dir(&self.root);
        }
    }

    fn manager(fixture: &Fixture, host: Arc<MockHost>) -> PluginActivationManager {
        let registry = Arc::new(PluginRegistry::new());
        registry
            .register(fixture.discovered.manifest.clone())
            .unwrap();
        PluginActivationManager::new(registry, host)
    }

    #[test]
    fn pinned_mock_plugin_activates_binds_and_stops() {
        let fixture = Fixture::new();
        let host = Arc::new(MockHost::default());
        let manager = manager(&fixture, host.clone());
        manager.activate(&fixture.discovered, &fixture.pin).unwrap();
        assert!(manager.activate(&fixture.discovered, &fixture.pin).is_err());
        assert_eq!(host.spawned.load(Ordering::SeqCst), 1);
        assert_eq!(manager.call_extension("echo", 7).unwrap(), 8);
        assert_eq!(
            manager.registry.get("echo").unwrap().unwrap().state,
            PluginState::Active
        );
        manager.stop("echo").unwrap();
        assert_eq!(host.spawned.load(Ordering::SeqCst), 1);
        assert_eq!(host.stopped.load(Ordering::SeqCst), 1);
        assert_eq!(
            manager.registry.get("echo").unwrap().unwrap().state,
            PluginState::Disabled
        );
    }

    #[test]
    fn changed_artifact_is_rejected_before_spawn() {
        let mut fixture = Fixture::new();
        fixture.pin.artifact_sha256 = "0".repeat(64);
        let host = Arc::new(MockHost::default());
        let first_manager = manager(&fixture, host.clone());
        assert!(first_manager
            .activate(&fixture.discovered, &fixture.pin)
            .is_err());
        assert_eq!(host.spawned.load(Ordering::SeqCst), 0);
        assert_eq!(
            first_manager.registry.get("echo").unwrap().unwrap().state,
            PluginState::Rejected
        );
    }

    #[test]
    fn crashed_plugin_is_unbound_and_marked_failed() {
        let fixture = Fixture::new();
        let host = Arc::new(MockHost::default());
        let manager = manager(&fixture, host.clone());
        manager.activate(&fixture.discovered, &fixture.pin).unwrap();
        host.crashed.store(true, Ordering::SeqCst);
        manager.refresh_health();
        assert_eq!(
            manager.registry.get("echo").unwrap().unwrap().state,
            PluginState::Failed
        );
        assert!(matches!(
            manager.call_extension("echo", 7),
            Err(ActivationError::NotActive(_))
        ));
        assert_eq!(host.stopped.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn correctly_signed_trusted_artifact_activates() {
        use ed25519_dalek::{Signer, SigningKey};

        let mut fixture = Fixture::new();
        fixture.discovered.manifest.trust_level = TrustLevel::Trusted;
        let manifest = serde_json::to_vec(&fixture.discovered.manifest).unwrap();
        std::fs::write(&fixture.pin.manifest_path, &manifest).unwrap();
        fixture.pin.manifest_sha256 =
            SignatureVerifier::compute_sha256_from_bytes(&manifest).hash_hex;
        fixture.pin.trust_level = TrustLevel::Trusted;
        let key = SigningKey::from_bytes(&[7u8; 32]);
        let public_key = hex::encode(key.verifying_key().to_bytes());
        let artifact = std::fs::read(&fixture.pin.artifact_path).unwrap();
        let signature = key.sign(&artifact);
        fixture.pin.trusted_public_key_hex = Some(public_key.clone());
        fixture.pin.signature = Some(PluginSignature::ed25519(
            hex::encode(signature.to_bytes()),
            public_key,
        ));

        let host = Arc::new(MockHost::default());
        let manager = manager(&fixture, host.clone());
        manager.activate(&fixture.discovered, &fixture.pin).unwrap();
        assert_eq!(manager.call_extension("echo", 5).unwrap(), 6);
        manager.stop("echo").unwrap();
        assert_eq!(host.spawned.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn unbound_interface_and_missing_trusted_signature_fail_closed() {
        let mut fixture = Fixture::new();
        fixture.discovered.manifest.interfaces = vec!["IGpuController".into()];
        let updated = serde_json::to_vec(&fixture.discovered.manifest).unwrap();
        std::fs::write(&fixture.pin.manifest_path, &updated).unwrap();
        fixture.pin.manifest_sha256 =
            SignatureVerifier::compute_sha256_from_bytes(&updated).hash_hex;
        let host = Arc::new(MockHost::default());
        let first_manager = manager(&fixture, host.clone());
        assert!(first_manager
            .activate(&fixture.discovered, &fixture.pin)
            .is_err());
        assert_eq!(host.spawned.load(Ordering::SeqCst), 0);

        let mut trusted = Fixture::new();
        trusted.discovered.manifest.trust_level = TrustLevel::Trusted;
        let bytes = serde_json::to_vec(&trusted.discovered.manifest).unwrap();
        std::fs::write(&trusted.pin.manifest_path, &bytes).unwrap();
        trusted.pin.manifest_sha256 = SignatureVerifier::compute_sha256_from_bytes(&bytes).hash_hex;
        trusted.pin.trust_level = TrustLevel::Trusted;
        trusted.pin.trusted_public_key_hex = Some("1".repeat(64));
        let manager = manager(&trusted, host.clone());
        assert!(manager.activate(&trusted.discovered, &trusted.pin).is_err());
        assert_eq!(host.spawned.load(Ordering::SeqCst), 0);
    }
}
