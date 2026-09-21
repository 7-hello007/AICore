//! AICore 统一错误类型。
//!
//! 本模块定义跨模块共享的 `AicoreError`。
//! 所有跨模块边界传播的错误必须使用此类型。
//!
//! ## 设计原则
//!
//! - 单一 enum，23 个变体，覆盖所有已知错误类别。
//! - 每个变体带一个稳定的 `ERR_*` 前缀，便于日志与 IPC 传输。
//! - 所有变体可 `Clone`，便于跨线程/跨进程传递。
//! - 兼容层：`ContractError` / `ContractResult` 为 type alias。
//!
//! ## 变体分类
//!
//! - 请求/契约：`InvalidRequest` / `ContractInvalid` / `NotFound` / `AlreadyExists`
//! - 权限/能力：`PermissionDenied` / `CapabilityUnavailable` / `ResourceUnavailable`
//!   / `AdapterUnavailable` / `NotSupported`
//! - AI：`AiInferenceFailed` / `AiTimeout`
//! - 应用：`AppNotRegistered`
//! - 配置：`ConfigInvalid`
//! - 插件：`PluginInvalid` / `PluginApiVersionMismatch` / `PluginFailed`
//!   / `PluginSignatureInvalid`
//! - Tier：`TierInvalid` / `TierLevelCannotControlNative`
//! - IPC：`IpcProtocolMismatch`
//! - 运行时：`Timeout` / `LockPoisoned`
//! - 通用：`Internal`

use thiserror::Error;

use crate::plugin::api_version::ApiVersion;
use crate::tier::level::TierLevel;

/// AICore 统一错误类型。
#[derive(Debug, Error, Clone, PartialEq)]
pub enum AicoreError {
    // ==================== 请求 / 契约 ====================
    #[error("ERR_INVALID_REQUEST: {0}")]
    InvalidRequest(String),

    #[error("ERR_CONTRACT_INVALID: {0}")]
    ContractInvalid(String),

    #[error("ERR_NOT_FOUND: {0}")]
    NotFound(String),

    #[error("ERR_ALREADY_EXISTS: {0}")]
    AlreadyExists(String),

    // ==================== 权限 / 能力 ====================
    #[error("ERR_PERMISSION_DENIED: {0}")]
    PermissionDenied(String),

    #[error("ERR_CAPABILITY_UNAVAILABLE: {0}")]
    CapabilityUnavailable(String),

    #[error("ERR_RESOURCE_UNAVAILABLE: {0}")]
    ResourceUnavailable(String),

    #[error("ERR_ADAPTER_UNAVAILABLE: {0}")]
    AdapterUnavailable(String),

    #[error("ERR_NOT_SUPPORTED: {0}")]
    NotSupported(String),

    // ==================== AI ====================
    #[error("ERR_AI_INFERENCE_FAILED: {0}")]
    AiInferenceFailed(String),

    #[error("ERR_AI_TIMEOUT: {0}")]
    AiTimeout(String),

    // ==================== 应用 ====================
    #[error("ERR_APP_NOT_REGISTERED: {0}")]
    AppNotRegistered(String),

    // ==================== 配置 ====================
    #[error("ERR_CONFIG_INVALID: {0}")]
    ConfigInvalid(String),

    // ==================== 插件 ====================
    #[error("ERR_PLUGIN_INVALID: {0}")]
    PluginInvalid(String),

    #[error("ERR_PLUGIN_API_VERSION_MISMATCH: plugin {plugin:?} vs core {core:?}")]
    PluginApiVersionMismatch {
        plugin: ApiVersion,
        core: ApiVersion,
    },

    #[error("ERR_PLUGIN_FAILED: {0}")]
    PluginFailed(String),

    #[error("ERR_PLUGIN_SIGNATURE_INVALID: {0}")]
    PluginSignatureInvalid(String),

    // ==================== Tier ====================
    #[error("ERR_TIER_INVALID: {0}")]
    TierInvalid(String),

    #[error("ERR_TIER_LEVEL_CANNOT_CONTROL_NATIVE: {0:?}")]
    TierLevelCannotControlNative(TierLevel),

    // ==================== IPC ====================
    #[error("ERR_PROTOCOL_MISMATCH: got {got}, expected {expected}")]
    IpcProtocolMismatch { got: String, expected: String },

    // ==================== 运行时 ====================
    #[error("ERR_RUNTIME_TIMEOUT: {0}")]
    Timeout(String),

    #[error("ERR_LOCK_POISONED: {0}")]
    LockPoisoned(String),

    // ==================== 通用 ====================
    #[error("ERR_INTERNAL: {0}")]
    Internal(String),
}

/// AICore 统一结果类型。
pub type AicoreResult<T> = Result<T, AicoreError>;

/// 兼容别名：`ContractError` = `AicoreError`。
///
/// 保留此别名，使现有代码 `ContractError::Xxx(...)` 无需修改。
/// 新代码应直接使用 `AicoreError`。
pub type ContractError = AicoreError;

/// 兼容别名：`ContractResult<T>` = `AicoreResult<T>`。
pub type ContractResult<T> = AicoreResult<T>;
