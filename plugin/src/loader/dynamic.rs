use std::ffi::CStr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use aicore_contracts::plugin::signature::PluginSignature;
use libloading::Library;

use super::abi::{ExecuteFn, NameFn, VersionFn, API_VERSION, SYM_EXECUTE, SYM_NAME, SYM_VERSION};
use crate::signature::{SignatureVerifier, SignatureVerifyError};

/// Loaded plugins.
///
/// **Note: `LoadedPlugin` has to stay alive or the bottom `Library` will be
/// Unmount, function pointer is invalid. Use `Arc<LoadedPlugin>` share.
pub struct LoadedPlugin {
    pub name: String,
    pub api_version: u32,
    pub source_path: PathBuf,
    pub signature: Option<PluginSignature>,

    /// Base library.** Field order is important.** - It must be before the function pointer.
    /// The `drop` pointer is guaranteed to expire first.
    _lib: Library,
    execute: ExecuteFn,
}

impl LoadedPlugin {
    /// Execute plugins.
    ///
    /// **Safety**: The caller must ensure the survival of `LoadedPlugin` throughout the call.
    /// `pub fn` secures this by borrowing `&self`.
    pub fn execute(&self, input: u32) -> u32 {
        unsafe { (self.execute)(input) }
    }
}

impl std::fmt::Debug for LoadedPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedPlugin")
            .field("name", &self.name)
            .field("api_version", &self.api_version)
            .field("source_path", &self.source_path)
            .field("signature", &self.signature)
            .finish()
    }
}

/// In-process loader for explicitly pinned trusted native code. Loading runs
/// native initialization before ABI inspection; this is not a security sandbox.
pub struct DynamicPluginLoader {
    /// Optional white list of signatures.
    ///
    /// `None`: Native loading is refused.
    /// - `Some(sig)`: Must match.
    expected_signature: Option<PluginSignature>,
}

impl DynamicPluginLoader {
    pub fn new() -> Self {
        Self {
            expected_signature: None,
        }
    }

    pub fn with_expected_signature(mut self, sig: PluginSignature) -> Self {
        self.expected_signature = Some(sig);
        self
    }

    pub fn expected_signature(&self) -> Option<&PluginSignature> {
        self.expected_signature.as_ref()
    }

    /// Load plugins.
    pub fn load(&self, path: impl AsRef<Path>) -> Result<Arc<LoadedPlugin>, LoadError> {
        let path = path.as_ref().to_path_buf();
        if !path.exists() {
            return Err(LoadError::NotFound(path));
        }

        // 1. Signature verification
        let expected = self
            .expected_signature
            .as_ref()
            .ok_or_else(|| {
                tracing::warn!(component="plugin_loader", event="trusted_pin_required", "in-process native plugin requires an explicit trusted signature pin; it is not sandboxed");
                LoadError::Signature(SignatureVerifyError::MalformedExpected)
            })?;
        SignatureVerifier::verify_file(&path, expected).map_err(LoadError::Signature)?;
        let signature = Some(expected.clone());

        // Opening the library
        let lib = unsafe { Library::new(&path).map_err(|e| LoadError::Open(e.to_string()))? };

        // 3. Finding symbols
        let (api_version, name, execute) = unsafe {
            let version: libloading::Symbol<VersionFn> = lib.get(SYM_VERSION).map_err(|e| {
                LoadError::MissingSymbol("aicore_plugin_api_version", e.to_string())
            })?;
            let name_sym: libloading::Symbol<NameFn> = lib
                .get(SYM_NAME)
                .map_err(|e| LoadError::MissingSymbol("aicore_plugin_name", e.to_string()))?;
            let exec: libloading::Symbol<ExecuteFn> = lib
                .get(SYM_EXECUTE)
                .map_err(|e| LoadError::MissingSymbol("aicore_plugin_execute", e.to_string()))?;

            let api_version = version();

            let name_ptr = name_sym();
            if name_ptr.is_null() {
                return Err(LoadError::InvalidName("null pointer".to_string()));
            }
            let name = CStr::from_ptr(name_ptr)
                .to_str()
                .map_err(|e| LoadError::InvalidName(e.to_string()))?
                .to_string();

            (api_version, name, *exec)
        };

        // Verify ABI
        if api_version != API_VERSION {
            return Err(LoadError::ApiVersionMismatch {
                expected: API_VERSION,
                got: api_version,
            });
        }

        tracing::info!(
            component = "plugin_loader",
            event = "loaded",
            name = %name,
            api_version = api_version,
            path = %path.display(),
            "plugin loaded"
        );

        Ok(Arc::new(LoadedPlugin {
            name,
            api_version,
            source_path: path,
            signature,
            _lib: lib,
            execute,
        }))
    }
}

impl Default for DynamicPluginLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum LoadError {
    #[error("plugin file not found: {0}")]
    NotFound(PathBuf),
    #[error("signature verify failed: {0}")]
    Signature(#[from] SignatureVerifyError),
    #[error("cannot open library: {0}")]
    Open(String),
    #[error("missing symbol `{0}`: {1}")]
    MissingSymbol(&'static str, String),
    #[error("invalid plugin name: {0}")]
    InvalidName(String),
    #[error("api version mismatch: expected {expected}, got {got}")]
    ApiVersionMismatch { expected: u32, got: u32 },
}
