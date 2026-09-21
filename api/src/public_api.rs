use std::sync::Arc;

use aicore_contracts::{
    application::{id::ApplicationId, registration::ApplicationRegistration},
    context::snapshot::ContextSnapshot,
    errors::{ContractError, ContractResult},
    task::contract::TaskContract,
};

use crate::context_merge::apply_task_contract;
use crate::registry::ApplicationRegistry;

/// 提交结果。
#[derive(Debug, Clone, PartialEq)]
pub struct SubmitResult {
    pub accepted: bool,
    pub task_id: Option<String>,
    pub message: String,
}

/// AICore 进程内公共 API。
///
/// 阶段七只做进程内 API；跨进程 IPC 由后续阶段引入。
pub struct PublicApi {
    registry: Arc<ApplicationRegistry>,
}

impl PublicApi {
    pub fn new(registry: Arc<ApplicationRegistry>) -> Self {
        Self { registry }
    }

    pub fn registry(&self) -> &Arc<ApplicationRegistry> {
        &self.registry
    }

    /// 注册应用。
    pub fn register(
        &self,
        application_id: ApplicationId,
        name: impl Into<String>,
        version: impl Into<String>,
        capabilities: Vec<String>,
    ) -> ContractResult<ApplicationRegistration> {
        self.registry
            .register(application_id, name, version, capabilities)
    }

    /// 注销应用。
    pub fn unregister(&self, application_id: &ApplicationId) -> ContractResult<bool> {
        self.registry.unregister(application_id)
    }

    /// 提交 TaskContract。
    ///
    /// 校验：
    /// - application_id 必须已注册
    /// - task_id 若提供，长度 ≤ 256
    /// - deadline_unix_secs 若提供，必须 > 0
    pub fn submit_task(&self, contract: &TaskContract) -> ContractResult<SubmitResult> {
        // 1. 应用必须已注册
        if !self.registry.is_registered(&contract.application_id)? {
            return Err(ContractError::AppNotRegistered(format!(
                "application not registered: {}",
                contract.application_id
            )));
        }

        // 2. task_id 合法性
        if let Some(tid) = &contract.task_id {
            if tid.trim().is_empty() || tid.len() > 256 {
                return Err(ContractError::ContractInvalid(
                    "task_id must be non-empty and <= 256 chars".to_string(),
                ));
            }
        }

        // 3. deadline 合法性
        if let Some(d) = contract.deadline_unix_secs {
            if d == 0 {
                return Err(ContractError::ContractInvalid(
                    "deadline_unix_secs must be > 0".to_string(),
                ));
            }
        }

        Ok(SubmitResult {
            accepted: true,
            task_id: contract.task_id.clone(),
            message: format!("accepted for {}", contract.application_id),
        })
    }

    /// 将契约应用到基础上下文。
    pub fn merge_context(
        &self,
        base: &ContextSnapshot,
        contract: &TaskContract,
    ) -> ContractResult<ContextSnapshot> {
        if !self.registry.is_registered(&contract.application_id)? {
            return Err(ContractError::AppNotRegistered(format!(
                "application not registered: {}",
                contract.application_id
            )));
        }
        Ok(apply_task_contract(base, contract))
    }
}
