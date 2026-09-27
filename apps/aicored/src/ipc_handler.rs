use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use aicore_ai::{validate_intent_proposal, BoundedIntentAgent};
use aicore_api::{ApplicationAction, ApplicationIdentityPolicy, ApplicationRegistry};
use aicore_contracts::dashboard::snapshot::DashboardSnapshot;
use aicore_contracts::task::contract::TaskContract;
use aicore_contracts::task::queued::QueuedTask;
use aicore_contracts::traits::native_task_provider::{INativeTaskProvider, NativeTaskHandle};
use aicore_contracts::traits::task_queue::ITaskQueue;
use aicore_contracts::{
    application::id::ApplicationId,
    capability::entry::CapabilityEntry,
    ipc::protocol::{IpcError, PluginEntryDto, Request, ResponseData},
    runtime::daemon_status::DaemonStatus,
};
use aicore_ipc::{ClientIdentity, IpcHandler};
use aicore_plugin::PluginRegistry;
use aicore_tier_l1::L1TaskSource;
use aicore_tier_l2::L2NativeTaskProvider;

use crate::tick_history::TickHistory;

/// Daemon 的 IPC 请求处理器。
pub struct DaemonIpcHandler {
    registry: Arc<ApplicationRegistry>,
    app_owners: Arc<Mutex<HashMap<ApplicationId, ClientIdentity>>>,
    identity_policy: Option<(
        Arc<ApplicationIdentityPolicy>,
        Arc<dyn IApplicationArtifactResolver>,
    )>,
    intent_agent: Option<Arc<BoundedIntentAgent>>,
    personal_memory: Option<Arc<dyn aicore_contracts::traits::memory_store::IMemoryStore>>,
    preparation_model_targets: Option<aicore_contracts::execution::targets::ExecutionTargets>,
    status: Arc<Mutex<DaemonStatus>>,
    history: Arc<TickHistory>,
    plugins: Arc<PluginRegistry>,
    task_queue: Arc<dyn ITaskQueue>,
    l1_source: Option<Arc<L1TaskSource>>,
    l2_provider: Option<Arc<L2NativeTaskProvider>>,
    capabilities: Vec<CapabilityEntry>,
    component: String,
    last_policy_json: Arc<Mutex<Option<String>>>,
    last_context_json: Arc<Mutex<Option<String>>>,
}

/// Platform adapters resolve artifact identity from the authenticated peer.
/// The request body and application name are never trusted as artifact proof.
pub trait IApplicationArtifactResolver: Send + Sync {
    fn artifact_sha256(&self, peer: ClientIdentity) -> Option<String>;
    fn principal(&self, _peer: ClientIdentity) -> Option<String> {
        None
    }
}

pub struct PeerExecutableArtifactResolver;
impl IApplicationArtifactResolver for PeerExecutableArtifactResolver {
    fn artifact_sha256(&self, peer: ClientIdentity) -> Option<String> {
        peer.artifact_sha256()
    }
    fn principal(&self, peer: ClientIdentity) -> Option<String> {
        peer.principal()
    }
}

fn current_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis().min(u64::MAX as u128) as u64)
        .unwrap_or(0)
}

struct ApplicationPreparationGuard {
    identity: ClientIdentity,
    application_id: ApplicationId,
    owners: Arc<Mutex<HashMap<ApplicationId, ClientIdentity>>>,
    memory: Arc<dyn aicore_contracts::traits::memory_store::IMemoryStore>,
    registry: Arc<ApplicationRegistry>,
    policy: Option<(
        Arc<ApplicationIdentityPolicy>,
        Arc<dyn IApplicationArtifactResolver>,
    )>,
    model_targets: Option<aicore_contracts::execution::targets::ExecutionTargets>,
    expires_at_unix_ms: u64,
}
impl aicore_contracts::traits::task_queue::IPreparationAuthorization
    for ApplicationPreparationGuard
{
    fn targets(
        &self,
    ) -> aicore_contracts::errors::ContractResult<
        aicore_contracts::execution::targets::ExecutionTargets,
    > {
        use aicore_contracts::errors::ContractError;
        if self
            .owners
            .lock()
            .map_err(|_| ContractError::LockPoisoned("application owner lock poisoned".into()))?
            .get(&self.application_id)
            != Some(&self.identity)
        {
            return Err(ContractError::PermissionDenied(
                "preparation application ownership changed".into(),
            ));
        }
        if !self.identity.is_current()
            || current_unix_ms() >= self.expires_at_unix_ms
            || !self.memory.load_profile()?.can_personalize()
            || !self.registry.is_registered(&self.application_id)?
        {
            return Err(ContractError::PermissionDenied(
                "preparation identity/consent expired or revoked".into(),
            ));
        }
        if let Some((policy, resolver)) = &self.policy {
            let digest = resolver.artifact_sha256(self.identity).ok_or_else(|| {
                ContractError::PermissionDenied("preparation artifact unavailable".into())
            })?;
            let principal = resolver.principal(self.identity);
            policy.authorize_identity(
                &self.application_id,
                &digest,
                principal.as_deref(),
                ApplicationAction::PrepareTask,
            )?;
        }
        let mut targets = match &self.model_targets {
            Some(targets) => targets.clone(),
            None => aicore_contracts::execution::targets::ExecutionTargets {
                foreground_pids: vec![self.identity.process_id().ok_or_else(|| {
                    ContractError::PermissionDenied("preparation requires a live peer PID".into())
                })?],
                background_pids: Vec::new(),
                process_only: true,
            },
        };
        targets.process_only = true;
        targets.background_pids.clear();
        if targets.foreground_pids.is_empty() {
            return Err(ContractError::ResourceUnavailable(
                "no preparation process target".into(),
            ));
        }
        Ok(targets)
    }
}

impl DaemonIpcHandler {
    /// Remove only confirmed dead/reused peers. Queue cancellation is done
    /// before removing the registration so departed apps cannot leave work
    /// waiting to be applied by a later tick.
    pub fn reap_stale_applications(&self) -> Result<usize, IpcError> {
        let mut owners = self
            .app_owners
            .lock()
            .map_err(|_| IpcError::internal("application owner lock poisoned"))?;
        let stale: Vec<_> = owners
            .iter()
            .filter(|(_, identity)| identity.is_stale())
            .map(|(app, _)| app.clone())
            .collect();
        for app in &stale {
            self.revoke_application(app)?;
            owners.remove(app);
        }
        Ok(stale.len())
    }

    fn revoke_application(&self, application_id: &ApplicationId) -> Result<(), IpcError> {
        for task in self
            .task_queue
            .snapshot()
            .map_err(|e| IpcError::internal(format!("task queue: {e}")))?
        {
            if task.contract.application_id == *application_id && task.state.is_active() {
                match self.task_queue.cancel(&task.task_id) {
                    Ok(()) => {},
                    Err(error) => {
                        let now = self
                            .task_queue
                            .get(&task.task_id)
                            .map_err(|e| IpcError::internal(format!("task queue: {e}")))?;
                        if now.is_some_and(|current| current.state.is_active()) {
                            return Err(IpcError::internal(format!(
                                "task cancellation failed: {error}"
                            )));
                        }
                    },
                }
            }
        }
        if let Some(source) = &self.l1_source {
            source
                .clear_for(application_id)
                .map_err(|e| IpcError::internal(format!("l1 clear: {e}")))?;
        }
        self.registry
            .unregister(application_id)
            .map_err(|e| IpcError::internal(format!("app registry: {e}")))?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        registry: Arc<ApplicationRegistry>,
        status: Arc<Mutex<DaemonStatus>>,
        history: Arc<TickHistory>,
        plugins: Arc<PluginRegistry>,
        task_queue: Arc<dyn ITaskQueue>,
        l1_source: Option<Arc<L1TaskSource>>,
        l2_provider: Option<Arc<L2NativeTaskProvider>>,
        capabilities: Vec<CapabilityEntry>,
        component: String,
        last_policy_json: Arc<Mutex<Option<String>>>,
        last_context_json: Arc<Mutex<Option<String>>>,
    ) -> Self {
        Self {
            registry,
            app_owners: Arc::new(Mutex::new(HashMap::new())),
            identity_policy: None,
            intent_agent: None,
            personal_memory: None,
            preparation_model_targets: None,
            status,
            history,
            plugins,
            task_queue,
            l1_source,
            l2_provider,
            capabilities,
            component,
            last_policy_json,
            last_context_json,
        }
    }

    pub fn with_identity_policy(
        mut self,
        policy: Arc<ApplicationIdentityPolicy>,
        resolver: Arc<dyn IApplicationArtifactResolver>,
    ) -> Self {
        self.identity_policy = Some((policy, resolver));
        self
    }

    pub fn with_intent_agent(mut self, agent: Arc<BoundedIntentAgent>) -> Self {
        self.intent_agent = Some(agent);
        self
    }
    pub fn with_personal_memory(
        mut self,
        memory: Arc<dyn aicore_contracts::traits::memory_store::IMemoryStore>,
        model_targets: Option<aicore_contracts::execution::targets::ExecutionTargets>,
    ) -> Self {
        self.personal_memory = Some(memory);
        self.preparation_model_targets = model_targets;
        self
    }

    fn authorize_artifact(
        &self,
        application_id: &ApplicationId,
        identity: ClientIdentity,
        action: ApplicationAction,
    ) -> Result<(), IpcError> {
        let Some((policy, resolver)) = &self.identity_policy else {
            return Ok(());
        };
        let digest = resolver.artifact_sha256(identity).ok_or_else(|| {
            IpcError::new(
                "ERR_PERMISSION_DENIED",
                "application artifact identity unavailable",
            )
        })?;
        let principal = resolver.principal(identity);
        policy
            .authorize_identity(application_id, &digest, principal.as_deref(), action)
            .map_err(|error| IpcError::new("ERR_PERMISSION_DENIED", error.to_string()))
    }

    fn authorize_app(
        &self,
        application_id: &ApplicationId,
        identity: ClientIdentity,
        action: ApplicationAction,
    ) -> Result<(), IpcError> {
        let owners = self
            .app_owners
            .lock()
            .map_err(|_| IpcError::internal("application owner lock poisoned"))?;
        if owners.get(application_id) == Some(&identity) {
            self.authorize_artifact(application_id, identity, action)
        } else {
            Err(IpcError::new(
                "ERR_PERMISSION_DENIED",
                "application is not bound to this IPC peer",
            ))
        }
    }

    fn register_bound(
        &self,
        application_id: ApplicationId,
        name: String,
        version: String,
        capabilities: Vec<String>,
        identity: ClientIdentity,
    ) -> Result<ResponseData, IpcError> {
        if identity == ClientIdentity::Unauthenticated {
            return Err(IpcError::new(
                "ERR_PERMISSION_DENIED",
                "IPC peer identity unavailable",
            ));
        }
        self.authorize_artifact(&application_id, identity, ApplicationAction::Register)?;
        let mut owners = self
            .app_owners
            .lock()
            .map_err(|_| IpcError::internal("application owner lock poisoned"))?;
        if owners
            .get(&application_id)
            .copied()
            .is_some_and(ClientIdentity::is_stale)
        {
            // Reclaim only a confirmed dead/reused OS identity. Clearing L1
            // prevents a departed app's declaration from influencing policy.
            self.revoke_application(&application_id)?;
            owners.remove(&application_id);
        }
        if owners
            .get(&application_id)
            .is_some_and(|owner| *owner != identity)
            || (!owners.contains_key(&application_id)
                && self
                    .registry
                    .is_registered(&application_id)
                    .map_err(|e| IpcError::internal(format!("app registry: {}", e)))?)
        {
            return Err(IpcError::new(
                "ERR_PERMISSION_DENIED",
                "application already registered by another peer",
            ));
        }
        self.registry
            .register(application_id.clone(), name, version, capabilities)
            .map_err(|e| IpcError::invalid_request(format!("{}", e)))?;
        owners.insert(application_id.clone(), identity);
        Ok(ResponseData::Registered {
            application_id: application_id.0,
        })
    }

    fn validate_registered_contract(
        &self,
        contract: &TaskContract,
        identity: ClientIdentity,
        action: ApplicationAction,
    ) -> Result<(), IpcError> {
        self.authorize_app(&contract.application_id, identity, action)?;
        if !self
            .registry
            .is_registered(&contract.application_id)
            .map_err(|e| IpcError::internal(format!("app registry: {}", e)))?
        {
            return Err(IpcError::new(
                "ERR_APP_NOT_REGISTERED",
                format!("application not registered: {}", contract.application_id),
            ));
        }
        if contract
            .task_id
            .as_ref()
            .is_some_and(|id| id.trim().is_empty() || id.len() > 256)
        {
            return Err(IpcError::invalid_request(
                "task_id must be non-empty and <= 256 chars",
            ));
        }
        if contract.deadline_unix_secs == Some(0) {
            return Err(IpcError::invalid_request("deadline_unix_secs must be > 0"));
        }
        Ok(())
    }
    fn dispatch(
        &self,
        request: Request,
        identity: ClientIdentity,
    ) -> Result<ResponseData, IpcError> {
        match request {
            Request::PrepareUpcomingTask {
                mut contract,
                starts_at_unix_ms,
            } => {
                self.validate_registered_contract(
                    &contract,
                    identity,
                    ApplicationAction::PrepareTask,
                )?;
                let memory = self
                    .personal_memory
                    .as_ref()
                    .ok_or_else(|| IpcError::not_supported("personal memory unavailable"))?;
                let profile = memory
                    .load_profile()
                    .map_err(|e| IpcError::internal(e.to_string()))?;
                if !profile.can_personalize() {
                    return Err(IpcError::new(
                        "ERR_PERMISSION_DENIED",
                        "personalization consent required",
                    ));
                }
                let now = current_unix_ms();
                if starts_at_unix_ms < now.saturating_add(1000)
                    || starts_at_unix_ms > now.saturating_add(300_000)
                {
                    return Err(IpcError::invalid_request(
                        "upcoming task must start within 1..300 seconds",
                    ));
                }
                let cutoff = now
                    .saturating_sub(u64::from(profile.retention_days).saturating_mul(86_400_000));
                let records: Vec<_> = memory
                    .load_records(1000)
                    .map_err(|e| IpcError::internal(e.to_string()))?
                    .into_iter()
                    .filter(|r| r.timestamp_unix_ms >= cutoff && r.timestamp_unix_ms <= now)
                    .collect();
                let aggregate =
                    aicore_contracts::memory::context::MemoryContext::from_records(&records);
                let hour = ((starts_at_unix_ms / 3_600_000) % 24) as u8;
                let pattern = aggregate
                    .application_patterns
                    .iter()
                    .find(|p| {
                        p.application_id == contract.application_id
                            && p.hour_utc == hour
                            && p.sample_count >= 3
                            && p.confidence >= 0.8
                    })
                    .ok_or_else(|| {
                        IpcError::not_supported("no reliable application pattern for upcoming time")
                    })?;
                if contract
                    .task
                    .is_some_and(|workload| workload != pattern.dominant_workload)
                {
                    return Err(IpcError::invalid_request(
                        "explicit task differs from personal prediction",
                    ));
                }
                contract.task = Some(pattern.dominant_workload);
                static PREPARATION_SEQUENCE: std::sync::atomic::AtomicU64 =
                    std::sync::atomic::AtomicU64::new(0);
                let seq = PREPARATION_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let id = format!("preparation-{now}-{seq}");
                contract.task_id = Some(id.clone());
                contract.deadline_unix_secs = Some(starts_at_unix_ms / 1000);
                let ready = starts_at_unix_ms.saturating_sub(30_000).max(now);
                let guard = ApplicationPreparationGuard {
                    identity,
                    application_id: contract.application_id.clone(),
                    owners: self.app_owners.clone(),
                    memory: memory.clone(),
                    registry: self.registry.clone(),
                    policy: self.identity_policy.clone(),
                    model_targets: self.preparation_model_targets.clone(),
                    expires_at_unix_ms: starts_at_unix_ms,
                };
                let preparation_id = self
                    .task_queue
                    .enqueue_preparation(QueuedTask::new(id, contract, now), ready, Arc::new(guard))
                    .map_err(|e| IpcError::invalid_request(e.to_string()))?;
                Ok(ResponseData::PreparationScheduled {
                    preparation_id,
                    workload: pattern.dominant_workload,
                    ready_at_unix_ms: ready,
                })
            },
            Request::Handshake { .. } => Err(IpcError::invalid_request(
                "handshake is handled by the IPC transport",
            )),
            Request::Register {
                application_id,
                name,
                version,
                capabilities,
            } => self.register_bound(application_id, name, version, capabilities, identity),

            Request::Unregister { application_id } => {
                let id = ApplicationId::new(application_id.0.clone());
                self.authorize_app(&id, identity, ApplicationAction::Unregister)?;
                let mut owners = self
                    .app_owners
                    .lock()
                    .map_err(|_| IpcError::internal("application owner lock poisoned"))?;
                if owners.get(&id) != Some(&identity) {
                    return Err(IpcError::new(
                        "ERR_PERMISSION_DENIED",
                        "application owner changed during unregister",
                    ));
                }
                self.revoke_application(&id)?;
                owners.remove(&id);
                Ok(ResponseData::Unregistered { removed: true })
            },

            Request::DeclareCurrentTask { contract } => {
                self.validate_registered_contract(
                    &contract,
                    identity,
                    ApplicationAction::DeclareTask,
                )?;
                let source = self
                    .l1_source
                    .as_ref()
                    .ok_or_else(|| IpcError::not_supported("L1 task source unavailable"))?;
                source
                    .set_current(contract)
                    .map_err(|e| IpcError::internal(format!("l1 declare: {}", e)))?;
                Ok(ResponseData::CurrentTaskDeclared)
            },

            Request::ClearCurrentTask { application_id } => {
                self.authorize_app(&application_id, identity, ApplicationAction::ClearTask)?;
                let source = self
                    .l1_source
                    .as_ref()
                    .ok_or_else(|| IpcError::not_supported("L1 task source unavailable"))?;
                let cleared = source
                    .clear_for(&application_id)
                    .map_err(|e| IpcError::internal(format!("l1 clear: {}", e)))?;
                Ok(ResponseData::CurrentTaskCleared { cleared })
            },

            Request::ProposeIntent { request } => {
                self.authorize_app(
                    &request.application_id,
                    identity,
                    ApplicationAction::ProposeIntent,
                )?;
                if request.text.trim().is_empty() || request.text.len() > 4096 {
                    return Err(IpcError::invalid_request("invalid intent text"));
                }
                let agent = self
                    .intent_agent
                    .as_ref()
                    .ok_or_else(|| IpcError::not_supported("local intent provider unavailable"))?;
                let proposal = aicore_contracts::traits::intent_agent::IIntentAgent::propose(
                    agent.as_ref(),
                    &request,
                )
                .map_err(|error| IpcError::new("ERR_AI_INFERENCE_FAILED", error.to_string()))?;
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|error| IpcError::internal(error.to_string()))?
                    .as_millis() as u64;
                validate_intent_proposal(&proposal, now)
                    .map_err(|error| IpcError::invalid_request(error.to_string()))?;
                Ok(ResponseData::IntentProposed { proposal })
            },

            Request::SubmitNativeTask { contract } => {
                self.validate_registered_contract(
                    &contract,
                    identity,
                    ApplicationAction::SubmitNativeTask,
                )?;
                let provider = self.l2_provider.as_ref().ok_or_else(|| {
                    IpcError::not_supported("L2 native task provider unavailable")
                })?;
                let handle = provider
                    .submit(&contract)
                    .map_err(|e| IpcError::internal(format!("l2 submit: {}", e)))?;
                Ok(ResponseData::NativeTaskSubmitted {
                    handle_id: handle.handle_id,
                })
            },

            Request::GetNativeTask {
                application_id,
                handle_id,
            } => {
                self.authorize_app(&application_id, identity, ApplicationAction::ReadNativeTask)?;
                let provider = self.l2_provider.as_ref().ok_or_else(|| {
                    IpcError::not_supported("L2 native task provider unavailable")
                })?;
                let handle = NativeTaskHandle { handle_id };
                let (contract, _, state) = provider
                    .task_info(&handle)
                    .map_err(|e| IpcError::invalid_request(format!("l2 query: {}", e)))?;
                if contract.application_id != application_id {
                    return Err(IpcError::new(
                        "ERR_PERMISSION_DENIED",
                        "task owner mismatch",
                    ));
                }
                Ok(ResponseData::NativeTaskStatus { state })
            },

            Request::CancelNativeTask {
                application_id,
                handle_id,
            } => {
                self.authorize_app(
                    &application_id,
                    identity,
                    ApplicationAction::CancelNativeTask,
                )?;
                let provider = self.l2_provider.as_ref().ok_or_else(|| {
                    IpcError::not_supported("L2 native task provider unavailable")
                })?;
                let handle = NativeTaskHandle { handle_id };
                let (contract, _, _) = provider
                    .task_info(&handle)
                    .map_err(|e| IpcError::invalid_request(format!("l2 query: {}", e)))?;
                if contract.application_id != application_id {
                    return Err(IpcError::new(
                        "ERR_PERMISSION_DENIED",
                        "task owner mismatch",
                    ));
                }
                provider
                    .cancel(&handle)
                    .map_err(|e| IpcError::invalid_request(format!("l2 cancel: {}", e)))?;
                Ok(ResponseData::NativeTaskCancelled)
            },

            Request::SubmitTask { contract } => {
                self.validate_registered_contract(
                    &contract,
                    identity,
                    ApplicationAction::SubmitTask,
                )?;
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                let queued = QueuedTask::new(
                    contract.task_id.clone().unwrap_or_default(),
                    contract.clone(),
                    now,
                );
                let assigned = self
                    .task_queue
                    .enqueue(queued)
                    .map_err(|e| IpcError::internal(format!("task queue: {}", e)))?;

                Ok(ResponseData::TaskSubmitted {
                    accepted: true,
                    task_id: Some(assigned),
                    message: format!("queued for {}", contract.application_id),
                })
            },

            Request::GetStatus => {
                let status = self
                    .status
                    .lock()
                    .map_err(|_| IpcError::internal("status lock poisoned"))?
                    .clone();
                Ok(ResponseData::Status { status })
            },

            Request::GetCapabilities => Ok(ResponseData::Capabilities {
                component: self.component.clone(),
                entries: self.capabilities.clone(),
            }),

            Request::GetContext => {
                let raw = self
                    .last_context_json
                    .lock()
                    .map_err(|_| IpcError::internal("context lock poisoned"))?
                    .clone();
                let context = raw.and_then(|s| serde_json::from_str(&s).ok());
                Ok(ResponseData::Context { context })
            },

            Request::GetPolicy => {
                let raw = self
                    .last_policy_json
                    .lock()
                    .map_err(|_| IpcError::internal("policy lock poisoned"))?
                    .clone();
                let policy = raw.and_then(|s| serde_json::from_str(&s).ok());
                Ok(ResponseData::Policy { policy })
            },

            Request::GetRecentTicks { limit } => {
                let ticks = self.history.snapshot(limit);
                Ok(ResponseData::RecentTicks { ticks })
            },

            Request::ListPlugins => {
                let entries = self
                    .plugins
                    .list()
                    .map_err(|e| IpcError::internal(format!("plugin registry: {}", e)))?;
                let dtos = entries.iter().map(plugin_entry_to_dto).collect();
                Ok(ResponseData::Plugins { entries: dtos })
            },

            Request::GetDashboardSnapshot => {
                let snapshot = self.build_dashboard_snapshot();
                Ok(ResponseData::DashboardSnapshot {
                    snapshot: Box::new(snapshot),
                })
            },
        }
    }
}

impl IpcHandler for DaemonIpcHandler {
    fn handle(&self, request: Request) -> Result<ResponseData, IpcError> {
        self.dispatch(request, ClientIdentity::Internal)
    }

    fn handle_with_identity(
        &self,
        request: Request,
        identity: ClientIdentity,
    ) -> Result<ResponseData, IpcError> {
        self.dispatch(request, identity)
    }

    fn name(&self) -> &'static str {
        "daemon_ipc_handler"
    }
}

fn plugin_entry_to_dto(e: &aicore_plugin::PluginEntry) -> PluginEntryDto {
    PluginEntryDto {
        name: e.manifest.name.clone(),
        version: e.manifest.version.clone(),
        api_version: e.manifest.api_version.clone(),
        kind: format!("{:?}", e.manifest.kind),
        trust_level: format!("{:?}", e.manifest.trust_level),
        state: format!("{:?}", e.state),
        interfaces: e.manifest.interfaces.clone(),
        permissions: e
            .manifest
            .permissions
            .iter()
            .map(|p| p.name().to_string())
            .collect(),
        last_reason: e.last_reason.clone(),
        failure_count: e.failure_count,
    }
}

impl DaemonIpcHandler {
    fn build_dashboard_snapshot(&self) -> DashboardSnapshot {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let mut snap = DashboardSnapshot::new(now);

        // 1. daemon status + 从 last_tick 提取的字段
        if let Ok(guard) = self.status.lock() {
            snap.daemon_status = Some(guard.clone());

            if let Some(last) = &guard.last_tick {
                snap.current_workload = Some(last.workload_type.clone());

                if let Some(json) = &last.current_context_json {
                    snap.current_context = serde_json::from_str(json).ok();
                }
                if let Some(json) = &last.current_policy_json {
                    snap.current_policy = serde_json::from_str(json).ok();
                }
                if let Some(json) = &last.runtime_route_json {
                    snap.runtime_route = serde_json::from_str(json).ok();
                }
                if let Some(json) = &last.ai_runtime_health_json {
                    snap.ai_runtime_health = serde_json::from_str(json).ok();
                }
                if let Some(pred) = last.predicted_workload {
                    snap.prediction =
                        Some(aicore_contracts::dashboard::snapshot::PredictionSummary {
                            predicted_workload: format!("{:?}", pred),
                            confidence: last.prediction_confidence.unwrap_or(0.0),
                            horizon_ms: None,
                        });
                }
                if let Some(es) = last.execution_status {
                    snap.execution_status =
                        Some(aicore_contracts::dashboard::snapshot::ExecutionSummary {
                            policy_id: last.policy_id.clone().unwrap_or_default(),
                            overall_status: format!("{:?}", es),
                            outcomes_count: 0,
                            duration_ms: last.duration_ms,
                        });
                }
            }
        }

        // 2. capabilities
        snap.capabilities = self.capabilities.clone();

        // 3. plugins
        if let Ok(entries) = self.plugins.list() {
            snap.plugin_states = entries.iter().map(plugin_entry_to_dto).collect();
        }

        // 4. recent ticks
        snap.recent_ticks = self.history.snapshot(10);

        snap
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aicore_api::ApprovedApplication;
    use aicore_contracts::traits::task_source::ITaskSource;
    use aicore_task_runtime::InMemoryTaskQueue;

    #[test]
    fn personal_preparation_actuates_model_process_and_rechecks_consent() {
        use aicore_contracts::{
            context::workload::{InteractionLevel, WorkloadType},
            execution::status::ExecutionOverallStatus,
            memory::{profile::UserProfile, record::HistoricalRecord},
            policy::{priority::PolicySource, resource_policy::PowerMode},
            traits::{memory_store::IMemoryStore, runtime_orchestrator::IRuntimeOrchestrator},
        };
        let memory = Arc::new(aicore_memory::InMemoryStore::new());
        let queue = Arc::new(InMemoryTaskQueue::new());
        let runtime = crate::runtime::build_runtime_for_mode(None, Some(queue.clone()), true);
        let app = ApplicationId::new("test.preparation");
        let now = current_unix_ms();
        let starts = now + 25_000;
        for day in 1..=3 {
            memory
                .append_record(&HistoricalRecord {
                    application_id: Some(app.clone()),
                    record_id: format!("prep-{day}"),
                    timestamp_unix_ms: (starts / 3_600_000) * 3_600_000 - day * 86_400_000,
                    workload_type: WorkloadType::Development,
                    interaction_level: InteractionLevel::Interactive,
                    policy_id: "old".into(),
                    policy_source: PolicySource::RuleBased,
                    overall_status: ExecutionOverallStatus::AllSucceeded,
                    cpu_usage_percent: 1.0,
                    gpu_usage_percent: None,
                    npu_usage_percent: None,
                    memory_usage_percent: 10.0,
                    power_mode: PowerMode::Balanced,
                })
                .unwrap();
        }
        let handler = DaemonIpcHandler::new(
            Arc::new(ApplicationRegistry::new()),
            Arc::new(Mutex::new(DaemonStatus::new(1))),
            Arc::new(TickHistory::with_default_capacity()),
            Arc::new(PluginRegistry::new()),
            queue.clone(),
            None,
            None,
            Vec::new(),
            "model".into(),
            Arc::new(Mutex::new(None)),
            Arc::new(Mutex::new(None)),
        )
        .with_personal_memory(memory.clone(), Some(runtime.execution_targets.clone()));
        handler
            .handle(Request::Register {
                application_id: app.clone(),
                name: "Test".into(),
                version: "1".into(),
                capabilities: Vec::new(),
            })
            .unwrap();
        let request = || Request::PrepareUpcomingTask {
            contract: TaskContract::minimal(app.clone()),
            starts_at_unix_ms: starts,
        };
        assert_eq!(
            handler.handle(request()).unwrap_err().code,
            "ERR_PERMISSION_DENIED"
        );
        memory
            .save_profile(&UserProfile {
                profile_enabled: true,
                ..Default::default()
            })
            .unwrap();
        let response = handler.handle(request()).unwrap();
        let ResponseData::PreparationScheduled { preparation_id, .. } = response else {
            panic!("preparation response required")
        };
        memory.save_profile(&UserProfile::default()).unwrap();
        let denied = runtime.loop_.tick(&runtime.execution_targets);
        assert!(!denied.success);
        assert_eq!(
            queue.get(&preparation_id).unwrap().unwrap().state,
            aicore_contracts::task::lifecycle::TaskState::Failed
        );
        memory
            .save_profile(&UserProfile {
                profile_enabled: true,
                ..Default::default()
            })
            .unwrap();
        let ResponseData::PreparationScheduled { preparation_id, .. } =
            handler.handle(request()).unwrap()
        else {
            panic!("preparation response required")
        };
        let applied = runtime.loop_.tick(&runtime.execution_targets);
        assert!(applied.success, "{}", applied.message);
        assert_eq!(
            queue.get(&preparation_id).unwrap().unwrap().state,
            aicore_contracts::task::lifecycle::TaskState::Completed
        );
        assert!(preparation_id.starts_with("preparation-"));
    }

    struct MockArtifactResolver;
    impl IApplicationArtifactResolver for MockArtifactResolver {
        fn artifact_sha256(&self, _peer: ClientIdentity) -> Option<String> {
            Some("a".repeat(64))
        }
    }

    struct MockLocalIntent;
    impl aicore_contracts::traits::intent_agent::IIntentAgent for MockLocalIntent {
        fn propose(
            &self,
            request: &aicore_contracts::ai::intent::IntentRequest,
        ) -> aicore_contracts::errors::ContractResult<aicore_contracts::ai::intent::IntentProposal>
        {
            Ok(aicore_contracts::ai::intent::IntentProposal {
                workload_type: aicore_contracts::context::workload::WorkloadType::Development,
                confidence: 0.9,
                reason: "mock local proposal".into(),
                expires_at_unix_ms: request.submitted_at_unix_ms + 30_000,
            })
        }
        fn deployment(&self) -> aicore_contracts::ai::intent::IntentDeployment {
            aicore_contracts::ai::intent::IntentDeployment::Local
        }
        fn name(&self) -> &'static str {
            "mock_local_intent"
        }
    }

    #[test]
    fn intent_api_requires_registered_pinned_app_and_returns_only_semantics() {
        let policy = Arc::new(ApplicationIdentityPolicy::new());
        let app = ApplicationId::new("app.intent_api");
        policy
            .pin(ApprovedApplication {
                application_id: app.clone(),
                artifact_sha256: "a".repeat(64),
                allowed_principals: Vec::new(),
                allowed_actions: vec![
                    ApplicationAction::Register,
                    ApplicationAction::ProposeIntent,
                ],
            })
            .unwrap();
        let agent = Arc::new(
            BoundedIntentAgent::new(
                Arc::new(MockLocalIntent),
                std::time::Duration::from_millis(100),
            )
            .unwrap(),
        );
        let handler = DaemonIpcHandler::new(
            Arc::new(ApplicationRegistry::new()),
            Arc::new(Mutex::new(DaemonStatus::new(1))),
            Arc::new(TickHistory::with_default_capacity()),
            Arc::new(PluginRegistry::new()),
            Arc::new(InMemoryTaskQueue::new()),
            None,
            None,
            Vec::new(),
            "mock".into(),
            Arc::new(Mutex::new(None)),
            Arc::new(Mutex::new(None)),
        )
        .with_identity_policy(policy, Arc::new(MockArtifactResolver))
        .with_intent_agent(agent);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let request = aicore_contracts::ai::intent::IntentRequest {
            application_id: app.clone(),
            text: "edit source".into(),
            submitted_at_unix_ms: now,
        };
        assert_eq!(
            handler
                .handle(Request::ProposeIntent {
                    request: request.clone()
                })
                .unwrap_err()
                .code,
            "ERR_PERMISSION_DENIED"
        );
        handler
            .handle(Request::Register {
                application_id: app,
                name: "Intent".into(),
                version: "1".into(),
                capabilities: vec![],
            })
            .unwrap();
        assert!(
            matches!(handler.handle(Request::ProposeIntent { request }).unwrap(),
            ResponseData::IntentProposed { proposal }
                if proposal.workload_type == aicore_contracts::context::workload::WorkloadType::Development)
        );
    }

    #[test]
    fn pinned_application_identity_denies_unapproved_action() {
        let policy = Arc::new(ApplicationIdentityPolicy::new());
        let app = ApplicationId::new("app.pinned");
        policy
            .pin(ApprovedApplication {
                application_id: app.clone(),
                artifact_sha256: "a".repeat(64),
                allowed_actions: vec![ApplicationAction::Register],
                allowed_principals: Vec::new(),
            })
            .unwrap();
        let handler = DaemonIpcHandler::new(
            Arc::new(ApplicationRegistry::new()),
            Arc::new(Mutex::new(DaemonStatus::new(1))),
            Arc::new(TickHistory::with_default_capacity()),
            Arc::new(PluginRegistry::new()),
            Arc::new(InMemoryTaskQueue::new()),
            None,
            None,
            Vec::new(),
            "mock".into(),
            Arc::new(Mutex::new(None)),
            Arc::new(Mutex::new(None)),
        )
        .with_identity_policy(policy.clone(), Arc::new(MockArtifactResolver));
        handler
            .handle(Request::Register {
                application_id: app.clone(),
                name: "Pinned".into(),
                version: "1".into(),
                capabilities: vec![],
            })
            .unwrap();
        let denied = handler
            .handle(Request::SubmitTask {
                contract: TaskContract::minimal(app.clone()),
            })
            .unwrap_err();
        assert_eq!(denied.code, "ERR_PERMISSION_DENIED");
        policy
            .pin(ApprovedApplication {
                application_id: app.clone(),
                artifact_sha256: "a".repeat(64),
                allowed_actions: vec![ApplicationAction::Register, ApplicationAction::SubmitTask],
                allowed_principals: Vec::new(),
            })
            .unwrap();
        assert!(matches!(
            handler
                .handle(Request::SubmitTask {
                    contract: TaskContract::minimal(app.clone()),
                })
                .unwrap(),
            ResponseData::TaskSubmitted { accepted: true, .. }
        ));
        policy.revoke(&app).unwrap();
        assert_eq!(
            handler
                .handle(Request::SubmitTask {
                    contract: TaskContract::minimal(app),
                })
                .unwrap_err()
                .code,
            "ERR_PERMISSION_DENIED"
        );
    }

    #[test]
    fn registered_application_can_declare_and_clear_l1_task() {
        let source = Arc::new(L1TaskSource::new());
        let handler = DaemonIpcHandler::new(
            Arc::new(ApplicationRegistry::new()),
            Arc::new(Mutex::new(DaemonStatus::new(1))),
            Arc::new(TickHistory::with_default_capacity()),
            Arc::new(PluginRegistry::new()),
            Arc::new(InMemoryTaskQueue::new()),
            Some(source.clone()),
            None,
            Vec::new(),
            "mock".to_string(),
            Arc::new(Mutex::new(None)),
            Arc::new(Mutex::new(None)),
        );
        let app = ApplicationId::new("app.l1");
        let contract = aicore_contracts::task::contract::TaskContract::minimal(app.clone());
        assert!(handler
            .handle(Request::DeclareCurrentTask {
                contract: contract.clone()
            })
            .is_err());
        handler
            .handle(Request::Register {
                application_id: app.clone(),
                name: "L1 test".to_string(),
                version: "1.0".to_string(),
                capabilities: Vec::new(),
            })
            .unwrap();
        assert!(matches!(
            handler
                .handle(Request::DeclareCurrentTask { contract })
                .unwrap(),
            ResponseData::CurrentTaskDeclared
        ));
        assert!(source.current_task().unwrap().is_some());
        assert!(handler
            .handle(Request::ClearCurrentTask {
                application_id: ApplicationId::new("other")
            })
            .is_err());
        assert!(matches!(
            handler
                .handle(Request::ClearCurrentTask {
                    application_id: app
                })
                .unwrap(),
            ResponseData::CurrentTaskCleared { cleared: true }
        ));
        assert!(source.current_task().unwrap().is_none());
    }

    #[test]
    fn l2_handle_tracks_the_shared_queue_and_checks_owner() {
        let queue: Arc<dyn ITaskQueue> = Arc::new(InMemoryTaskQueue::new());
        let provider = Arc::new(L2NativeTaskProvider::new().with_task_queue(queue.clone()));
        let handler = DaemonIpcHandler::new(
            Arc::new(ApplicationRegistry::new()),
            Arc::new(Mutex::new(DaemonStatus::new(1))),
            Arc::new(TickHistory::with_default_capacity()),
            Arc::new(PluginRegistry::new()),
            queue.clone(),
            None,
            Some(provider),
            Vec::new(),
            "mock".to_string(),
            Arc::new(Mutex::new(None)),
            Arc::new(Mutex::new(None)),
        );
        let app = ApplicationId::new("app.l2");
        handler
            .handle(Request::Register {
                application_id: app.clone(),
                name: "L2 test".to_string(),
                version: "1.0".to_string(),
                capabilities: Vec::new(),
            })
            .unwrap();
        let contract = aicore_contracts::task::contract::TaskContract::minimal(app.clone());
        let handle_id = match handler
            .handle(Request::SubmitNativeTask { contract })
            .unwrap()
        {
            ResponseData::NativeTaskSubmitted { handle_id } => handle_id,
            other => panic!("unexpected response: {:?}", other),
        };
        assert!(handler
            .handle(Request::GetNativeTask {
                application_id: ApplicationId::new("other"),
                handle_id: handle_id.clone(),
            })
            .is_err());
        let running = queue.dequeue().unwrap().unwrap();
        assert_eq!(running.task_id, handle_id);
        queue.complete(&handle_id, None).unwrap();
        assert!(matches!(
            handler
                .handle(Request::GetNativeTask {
                    application_id: app,
                    handle_id,
                })
                .unwrap(),
            ResponseData::NativeTaskStatus {
                state: aicore_contracts::task::lifecycle::TaskState::Completed
            }
        ));
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn app_task_lifecycle_roundtrips_over_isolated_ipc() {
        use aicore_ipc::{unwrap_response, IpcClient, IpcServer};
        use std::time::Duration;
        use tokio::sync::mpsc;

        let queue: Arc<dyn ITaskQueue> = Arc::new(InMemoryTaskQueue::new());
        let source = Arc::new(L1TaskSource::new());
        let provider = Arc::new(L2NativeTaskProvider::new().with_task_queue(queue.clone()));
        let handler: Arc<dyn IpcHandler> = Arc::new(DaemonIpcHandler::new(
            Arc::new(ApplicationRegistry::new()),
            Arc::new(Mutex::new(DaemonStatus::new(1))),
            Arc::new(TickHistory::with_default_capacity()),
            Arc::new(PluginRegistry::new()),
            queue.clone(),
            Some(source.clone()),
            Some(provider),
            Vec::new(),
            "mock".to_string(),
            Arc::new(Mutex::new(None)),
            Arc::new(Mutex::new(None)),
        ));
        let path = std::env::temp_dir().join(format!(
            "aicore-l2-ipc-{}-{}.sock",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let (shutdown, rx) = mpsc::channel(1);
        let server = IpcServer::bind(&path, handler).unwrap().with_shutdown(rx);
        let task = tokio::spawn(server.run());
        let mut client = IpcClient::connect(&path).await.unwrap();
        let app = ApplicationId::new("app.ipc");
        let timeout = Duration::from_secs(2);

        let registered = client
            .call(
                "register",
                Request::Register {
                    application_id: app.clone(),
                    name: "IPC test".to_string(),
                    version: "1.0".to_string(),
                    capabilities: Vec::new(),
                },
                timeout,
            )
            .await
            .unwrap();
        assert!(matches!(
            unwrap_response(registered).unwrap(),
            ResponseData::Registered { .. }
        ));

        let contract = TaskContract::minimal(app.clone());
        let declared = client
            .call(
                "declare",
                Request::DeclareCurrentTask {
                    contract: contract.clone(),
                },
                timeout,
            )
            .await
            .unwrap();
        assert!(matches!(
            unwrap_response(declared).unwrap(),
            ResponseData::CurrentTaskDeclared
        ));
        assert!(source.current_task().unwrap().is_some());

        let submitted = client
            .call(
                "submit-native",
                Request::SubmitNativeTask { contract },
                timeout,
            )
            .await
            .unwrap();
        let handle_id = match unwrap_response(submitted).unwrap() {
            ResponseData::NativeTaskSubmitted { handle_id } => handle_id,
            other => panic!("unexpected response: {:?}", other),
        };
        assert_eq!(
            queue.get(&handle_id).unwrap().unwrap().state,
            aicore_contracts::task::lifecycle::TaskState::Queued
        );

        let cancelled = client
            .call(
                "cancel-native",
                Request::CancelNativeTask {
                    application_id: app.clone(),
                    handle_id: handle_id.clone(),
                },
                timeout,
            )
            .await
            .unwrap();
        assert!(matches!(
            unwrap_response(cancelled).unwrap(),
            ResponseData::NativeTaskCancelled
        ));
        let queried = client
            .call(
                "query-native",
                Request::GetNativeTask {
                    application_id: app.clone(),
                    handle_id,
                },
                timeout,
            )
            .await
            .unwrap();
        assert!(matches!(
            unwrap_response(queried).unwrap(),
            ResponseData::NativeTaskStatus {
                state: aicore_contracts::task::lifecycle::TaskState::Cancelled
            }
        ));

        let cleared = client
            .call(
                "clear",
                Request::ClearCurrentTask {
                    application_id: app,
                },
                timeout,
            )
            .await
            .unwrap();
        assert!(matches!(
            unwrap_response(cleared).unwrap(),
            ResponseData::CurrentTaskCleared { cleared: true }
        ));
        assert!(source.current_task().unwrap().is_none());

        shutdown.send(()).await.unwrap();
        task.await.unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn dead_peer_binding_is_reclaimed_and_its_l1_task_cleared() {
        let source = Arc::new(L1TaskSource::new());
        let handler = DaemonIpcHandler::new(
            Arc::new(ApplicationRegistry::new()),
            Arc::new(Mutex::new(DaemonStatus::new(1))),
            Arc::new(TickHistory::with_default_capacity()),
            Arc::new(PluginRegistry::new()),
            Arc::new(InMemoryTaskQueue::new()),
            Some(source.clone()),
            None,
            Vec::new(),
            "mock".into(),
            Arc::new(Mutex::new(None)),
            Arc::new(Mutex::new(None)),
        );
        let app = ApplicationId::new("app.reconnect");
        let departed = ClientIdentity::Linux {
            uid: 1000,
            pid: i32::MAX,
            start_time_ticks: 1,
            executable_dev: 0,
            executable_ino: 0,
        };
        let successor = ClientIdentity::Linux {
            uid: 1000,
            pid: i32::MAX - 1,
            start_time_ticks: 2,
            executable_dev: 0,
            executable_ino: 0,
        };
        let register = || Request::Register {
            application_id: app.clone(),
            name: "reconnect".into(),
            version: "1.0".into(),
            capabilities: Vec::new(),
        };
        handler.handle_with_identity(register(), departed).unwrap();
        handler
            .handle_with_identity(
                Request::DeclareCurrentTask {
                    contract: TaskContract::minimal(app.clone()),
                },
                departed,
            )
            .unwrap();
        assert!(source.current_task().unwrap().is_some());
        handler.handle_with_identity(register(), successor).unwrap();
        assert!(source.current_task().unwrap().is_none());
        assert!(handler
            .handle_with_identity(
                Request::SubmitTask {
                    contract: TaskContract::minimal(app),
                },
                departed
            )
            .is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn stale_reaper_cancels_departed_app_work_without_new_registration() {
        let queue: Arc<dyn ITaskQueue> = Arc::new(InMemoryTaskQueue::new());
        let source = Arc::new(L1TaskSource::new());
        let registry = Arc::new(ApplicationRegistry::new());
        let handler = DaemonIpcHandler::new(
            registry.clone(),
            Arc::new(Mutex::new(DaemonStatus::new(1))),
            Arc::new(TickHistory::with_default_capacity()),
            Arc::new(PluginRegistry::new()),
            queue.clone(),
            Some(source.clone()),
            None,
            Vec::new(),
            "mock".into(),
            Arc::new(Mutex::new(None)),
            Arc::new(Mutex::new(None)),
        );
        let app = ApplicationId::new("app.departed");
        let owner = ClientIdentity::Linux {
            uid: 1000,
            pid: i32::MAX,
            start_time_ticks: 1,
            executable_dev: 0,
            executable_ino: 0,
        };
        handler
            .handle_with_identity(
                Request::Register {
                    application_id: app.clone(),
                    name: "departed".into(),
                    version: "1.0".into(),
                    capabilities: Vec::new(),
                },
                owner,
            )
            .unwrap();
        let contract = TaskContract::minimal(app.clone());
        handler
            .handle_with_identity(
                Request::DeclareCurrentTask {
                    contract: contract.clone(),
                },
                owner,
            )
            .unwrap();
        let task_id = match handler
            .handle_with_identity(Request::SubmitTask { contract }, owner)
            .unwrap()
        {
            ResponseData::TaskSubmitted { task_id, .. } => task_id.unwrap(),
            other => panic!("unexpected response: {other:?}"),
        };
        assert_eq!(handler.reap_stale_applications().unwrap(), 1);
        assert_eq!(handler.reap_stale_applications().unwrap(), 0);
        assert_eq!(
            queue.get(&task_id).unwrap().unwrap().state,
            aicore_contracts::task::lifecycle::TaskState::Cancelled
        );
        assert!(source.current_task().unwrap().is_none());
        assert!(!registry.is_registered(&app).unwrap());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_second_peer_cannot_claim_or_act_for_registered_application() {
        use std::os::unix::fs::MetadataExt;
        let queue: Arc<dyn ITaskQueue> = Arc::new(InMemoryTaskQueue::new());
        let source = Arc::new(L1TaskSource::new());
        let provider = Arc::new(L2NativeTaskProvider::new().with_task_queue(queue.clone()));
        let handler = DaemonIpcHandler::new(
            Arc::new(ApplicationRegistry::new()),
            Arc::new(Mutex::new(DaemonStatus::new(1))),
            Arc::new(TickHistory::with_default_capacity()),
            Arc::new(PluginRegistry::new()),
            queue,
            Some(source),
            Some(provider),
            Vec::new(),
            "mock".to_string(),
            Arc::new(Mutex::new(None)),
            Arc::new(Mutex::new(None)),
        );
        let uid = std::fs::metadata("/proc/self").unwrap().uid();
        let pid = std::process::id() as i32;
        let start_time_ticks = std::fs::read_to_string("/proc/self/stat")
            .unwrap()
            .rsplit_once(") ")
            .unwrap()
            .1
            .split_whitespace()
            .nth(19)
            .unwrap()
            .parse::<u64>()
            .unwrap();
        let executable = std::fs::metadata("/proc/self/exe").unwrap();
        let owner = ClientIdentity::Linux {
            uid,
            pid,
            start_time_ticks,
            executable_dev: executable.dev(),
            executable_ino: executable.ino(),
        };
        let impostor = ClientIdentity::Linux {
            uid,
            pid,
            start_time_ticks: start_time_ticks + 1,
            executable_dev: executable.dev(),
            executable_ino: executable.ino(),
        };
        let app = ApplicationId::new("app.bound");
        let register = || Request::Register {
            application_id: app.clone(),
            name: "bound".to_string(),
            version: "1.0".to_string(),
            capabilities: Vec::new(),
        };
        handler.handle_with_identity(register(), owner).unwrap();
        assert!(handler.handle_with_identity(register(), impostor).is_err());
        let contract = TaskContract::minimal(app.clone());
        assert!(handler
            .handle_with_identity(
                Request::DeclareCurrentTask {
                    contract: contract.clone()
                },
                impostor
            )
            .is_err());
        assert!(handler
            .handle_with_identity(
                Request::SubmitTask {
                    contract: contract.clone()
                },
                impostor
            )
            .is_err());
        assert!(handler
            .handle_with_identity(Request::SubmitNativeTask { contract }, impostor)
            .is_err());
        assert!(handler
            .handle_with_identity(
                Request::ClearCurrentTask {
                    application_id: app.clone()
                },
                impostor
            )
            .is_err());
        assert!(handler
            .handle_with_identity(
                Request::Unregister {
                    application_id: app
                },
                impostor
            )
            .is_err());
    }
}
