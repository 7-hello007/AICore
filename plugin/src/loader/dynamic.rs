use std::ffi::CStr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use aicore_contracts::plugin::signature::PluginSignature;
use libloading::Library;

use super::abi::{ExecuteFn, NameFn, VersionFn, API_VERSION, SYM_EXECUTE, SYM_NAME, SYM_VERSION};
use crate::signature::{SignatureVerifier, SignatureVerifyError};

/// 已加载的插件。
///
/// **注意**：`LoadedPlugin` 必须保持存活，否则底层 `Library` 会被
/// 卸载，函数指针失效。使用 `Arc<LoadedPlugin>` 共享。
pub struct LoadedPlugin {
    pub name: String,
    pub api_version: u32,
    pub source_path: PathBuf,
    pub signature: Option<PluginSignature>,

    /// 底层库。**字段顺序很重要**——必须在函数指针之前，
    /// 保证 `drop` 时函数指针先失效。
    _lib: Library,
    execute: ExecuteFn,
}

impl LoadedPlugin {
    /// 执行插件。
    ///
    /// **安全性**：调用方必须保证此 `LoadedPlugin` 在整个调用期间存活。
    /// `pub fn` 通过 `&self` 借用自动保证这一点。
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

/// 动态插件加载器。
pub struct DynamicPluginLoader {
    /// 可选的签名白名单。
    ///
    /// - `None`：不校验签名（开发模式）。
    /// - `Some(sig)`：必须匹配。
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

    /// 加载插件。
    pub fn load(&self, path: impl AsRef<Path>) -> Result<Arc<LoadedPlugin>, LoadError> {
        let path = path.as_ref().to_path_buf();
        if !path.exists() {
            return Err(LoadError::NotFound(path));
        }

        // 1. 签名校验
        let signature = if let Some(expected) = &self.expected_signature {
            SignatureVerifier::verify_file(&path, expected).map_err(LoadError::Signature)?;
            Some(expected.clone())
        } else {
            // 未配置签名 → 只计算（用于审计）
            SignatureVerifier::compute_sha256(&path).ok()
        };

        // 2. 打开库
        let lib = unsafe { Library::new(&path).map_err(|e| LoadError::Open(e.to_string()))? };

        // 3. 查找符号
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

        // 4. ABI 版本校验
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
