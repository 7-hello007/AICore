use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use aicore_contracts::{
    ai::runtime::{
        InferenceRequest, InferenceResponse, ModelHandle, ModelSpec, RuntimeCapability,
        RuntimeHealth,
    },
    errors::{ContractError, ContractResult},
    traits::ai_runtime::IAiRuntime,
};

/// 带 fallback 的 Runtime 组合器。
///
/// 语义：
/// - `primary` 优先。
/// - `primary` 任何错误（load / infer / unload）→ 透明转 `fallback`。
/// - `primary` 与 `fallback` 都失败 → 返回 `primary` 的错误。
/// - 记录 fallback 次数，供审计。
pub struct FallbackAiRuntime {
    primary: Arc<dyn IAiRuntime>,
    fallback: Arc<dyn IAiRuntime>,
    fallback_load_count: AtomicU64,
    fallback_infer_count: AtomicU64,
}

impl FallbackAiRuntime {
    pub fn new(primary: Arc<dyn IAiRuntime>, fallback: Arc<dyn IAiRuntime>) -> Self {
        Self {
            primary,
            fallback,
            fallback_load_count: AtomicU64::new(0),
            fallback_infer_count: AtomicU64::new(0),
        }
    }

    pub fn primary_name(&self) -> &'static str {
        self.primary.runtime_name()
    }

    pub fn fallback_name(&self) -> &'static str {
        self.fallback.runtime_name()
    }

    pub fn fallback_load_count(&self) -> u64 {
        self.fallback_load_count.load(Ordering::SeqCst)
    }

    pub fn fallback_infer_count(&self) -> u64 {
        self.fallback_infer_count.load(Ordering::SeqCst)
    }
}

impl IAiRuntime for FallbackAiRuntime {
    fn load_model(&self, spec: &ModelSpec) -> ContractResult<ModelHandle> {
        match self.primary.load_model(spec) {
            Ok(h) => Ok(h),
            Err(primary_err) => {
                tracing::warn!(
                    component = "fallback_runtime",
                    event = "load_fallback",
                    primary = self.primary.runtime_name(),
                    fallback = self.fallback.runtime_name(),
                    error = %primary_err,
                    "primary load failed; trying fallback"
                );
                match self.fallback.load_model(spec) {
                    Ok(h) => {
                        self.fallback_load_count.fetch_add(1, Ordering::SeqCst);
                        Ok(h)
                    },
                    Err(fallback_err) => {
                        tracing::error!(
                            component = "fallback_runtime",
                            event = "load_all_failed",
                            primary_error = %primary_err,
                            fallback_error = %fallback_err,
                            "both primary and fallback failed"
                        );
                        Err(ContractError::AiInferenceFailed(format!(
                            "primary: {}; fallback: {}",
                            primary_err, fallback_err
                        )))
                    },
                }
            },
        }
    }

    fn unload(&self, handle: &ModelHandle) -> ContractResult<()> {
        // 尝试主 runtime
        if let Ok(()) = self.primary.unload(handle) {
            return Ok(());
        }
        // 回退
        self.fallback.unload(handle)
    }

    fn infer(
        &self,
        handle: &ModelHandle,
        request: &InferenceRequest,
    ) -> ContractResult<InferenceResponse> {
        match self.primary.infer(handle, request) {
            Ok(r) => Ok(r),
            Err(primary_err) => {
                tracing::warn!(
                    component = "fallback_runtime",
                    event = "infer_fallback",
                    primary = self.primary.runtime_name(),
                    fallback = self.fallback.runtime_name(),
                    error = %primary_err,
                    "primary infer failed; trying fallback"
                );
                match self.fallback.infer(handle, request) {
                    Ok(r) => {
                        self.fallback_infer_count.fetch_add(1, Ordering::SeqCst);
                        Ok(r)
                    },
                    Err(fallback_err) => Err(ContractError::AiInferenceFailed(format!(
                        "primary: {}; fallback: {}",
                        primary_err, fallback_err
                    ))),
                }
            },
        }
    }

    fn capabilities(&self) -> Vec<RuntimeCapability> {
        let mut caps = self.primary.capabilities();
        for c in self.fallback.capabilities() {
            if !caps.contains(&c) {
                caps.push(c);
            }
        }
        caps
    }

    fn health(&self) -> RuntimeHealth {
        // 主 runtime 健康则报主；否则报 fallback 的健康
        match self.primary.health() {
            RuntimeHealth::Healthy { loaded_models } if loaded_models > 0 => {
                RuntimeHealth::Healthy { loaded_models }
            },
            _ => self.fallback.health(),
        }
    }

    fn runtime_name(&self) -> &'static str {
        "fallback_runtime"
    }
}
