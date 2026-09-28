use std::sync::Arc;

use aicore_contracts::{
    application::{id::ApplicationId, registration::ApplicationRegistration},
    context::snapshot::ContextSnapshot,
    errors::{ContractError, ContractResult},
    task::{contract::TaskContract, queued::QueuedTask},
    traits::task_queue::ITaskQueue,
};

use crate::context_merge::apply_task_contract;
use crate::registry::ApplicationRegistry;

/// Submission of results.
#[derive(Debug, Clone, PartialEq)]
pub struct SubmitResult {
    pub accepted: bool,
    pub task_id: Option<String>,
    pub message: String,
}

/// Public API within the AICore process.
///
/// Phase 7 is limited to API; cross-process IPC is introduced from the follow-up phase.
pub struct PublicApi {
    registry: Arc<ApplicationRegistry>,
    task_queue: Arc<dyn ITaskQueue>,
}

impl PublicApi {
    pub fn new(registry: Arc<ApplicationRegistry>) -> Self {
        Self {
            registry,
            task_queue: Arc::new(aicore_task_runtime::InMemoryTaskQueue::new()),
        }
    }

    /// Use the shared TaskQueue already available in daemon to ensure that API has the same syntax as IPC in the process.
    pub fn with_task_queue(mut self, task_queue: Arc<dyn ITaskQueue>) -> Self {
        self.task_queue = task_queue;
        self
    }

    pub fn task_queue(&self) -> &Arc<dyn ITaskQueue> {
        &self.task_queue
    }

    pub fn registry(&self) -> &Arc<ApplicationRegistry> {
        &self.registry
    }

    /// Register application.
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

    /// Write-off application.
    pub fn unregister(&self, application_id: &ApplicationId) -> ContractResult<bool> {
        self.registry.unregister(application_id)
    }

    /// Submit TaskContract.
    ///
    /// Verify:
    /// - application_id must be registered.
    /// - task_id, if available, length 256
    /// - deadline_unix_secs, if available, must > 0
    pub fn submit_task(&self, contract: &TaskContract) -> ContractResult<SubmitResult> {
        // Applications must be registered
        if !self.registry.is_registered(&contract.application_id)? {
            return Err(ContractError::AppNotRegistered(format!(
                "application not registered: {}",
                contract.application_id
            )));
        }

        // 2. task_id Legitimacy
        if let Some(tid) = &contract.task_id {
            if tid.trim().is_empty() || tid.len() > 256 {
                return Err(ContractError::ContractInvalid(
                    "task_id must be non-empty and <= 256 chars".to_string(),
                ));
            }
        }

        // 3. Deadline Legitimacy
        if let Some(d) = contract.deadline_unix_secs {
            if d == 0 {
                return Err(ContractError::ContractInvalid(
                    "deadline_unix_secs must be > 0".to_string(),
                ));
            }
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let queued = QueuedTask::new(
            contract.task_id.clone().unwrap_or_default(),
            contract.clone(),
            now,
        );
        let assigned = self.task_queue.enqueue(queued)?;

        Ok(SubmitResult {
            accepted: true,
            task_id: Some(assigned),
            message: format!("queued for {}", contract.application_id),
        })
    }

    /// Applies the compact to the following.
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
