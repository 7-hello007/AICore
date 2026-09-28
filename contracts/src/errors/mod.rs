//! AICore's shared error types.
//!
//! This module defines `AicoreError`, which is shared across modules.
//! Errors propagated across module boundaries must use this type.
//!
//! # Design principles
//!
//! - One enum with 23 variants covers the known error categories.
//! - Each variant has a stable `ERR_*` prefix for logs and IPC.
//! - Every variant implements `Clone` for transfer across threads and processes.
//! - `ContractError` and `ContractResult` remain compatibility aliases.
//!
//! # Variant categories
//!
//! - Request/contract: `InvalidRequest`/`ContractInvalid`/`NotFound`/`AlreadyExists`
//! - Permission/capability: `PermissionDenied`/`CapabilityUnavailable`/`ResourceUnavailable`
//!   / `AdapterUnavailable` / `NotSupported`
//! - AI: `AiInferenceFailed` / `AiTimeout`
//! - Application: `AppNotRegistered`
//! - Configuration: `ConfigInvalid`
//! - Plugin: `PluginInvalid`/`PluginApiVersionMismatch`/`PluginFailed`
//!   / `PluginSignatureInvalid`
//! - Tier: `TierInvalid` / `TierLevelCannotControlNative`
//! - IPC: `IpcProtocolMismatch`
//! - Runtime: `Timeout`/`LockPoisoned`
//! - General: `Internal`

use thiserror::Error;

use crate::plugin::api_version::ApiVersion;
use crate::tier::level::TierLevel;

/// AICore's shared error type.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum AicoreError {
    // ==================== Request / contract ====================
    #[error("ERR_INVALID_REQUEST: {0}")]
    InvalidRequest(String),

    #[error("ERR_CONTRACT_INVALID: {0}")]
    ContractInvalid(String),

    #[error("ERR_NOT_FOUND: {0}")]
    NotFound(String),

    #[error("ERR_ALREADY_EXISTS: {0}")]
    AlreadyExists(String),

    // ==================== Permission / capability ====================
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

    // ==================== Application ====================
    #[error("ERR_APP_NOT_REGISTERED: {0}")]
    AppNotRegistered(String),

    // ==================== Configuration ====================
    #[error("ERR_CONFIG_INVALID: {0}")]
    ConfigInvalid(String),

    // ==================== Plugin ====================
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

    // ==================== Runtime ====================
    #[error("ERR_RUNTIME_TIMEOUT: {0}")]
    Timeout(String),

    #[error("ERR_LOCK_POISONED: {0}")]
    LockPoisoned(String),

    // ==================== General ====================
    #[error("ERR_INTERNAL: {0}")]
    Internal(String),
}

/// AICore's shared result type.
pub type AicoreResult<T> = Result<T, AicoreError>;

/// Compatibility alias: `ContractError` = `AicoreError`.
///
/// Keep this alias so existing `ContractError::Xxx(...)` call sites remain valid.
/// New code should use `AicoreError` directly.
pub type ContractError = AicoreError;

/// Compatibility alias: `ContractResult<T>` = `AicoreResult<T>`.
pub type ContractResult<T> = AicoreResult<T>;
