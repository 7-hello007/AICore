//! Disposable request preparation plus bounded, acknowledged host-state commits.
//! Worker code never receives pointers to the daemon's mutable state.
use super::*;
use aicore_api::ApprovedApplication;
use aicore_contracts::{
    application::registration::ApplicationRegistration,
    ipc::protocol::{Envelope, ResponseEnvelope},
    memory::{profile::UserProfile, record::HistoricalRecord},
    runtime::daemon_status::TickSummary,
    traits::memory_store::{IMemoryStore, ProcessMemorySource},
};
use aicore_ipc::process_handler::{ProcessHandlerRequest, ProcessHandlerSpec};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, time::Instant};

#[derive(Clone, Serialize, Deserialize)]
struct ReadState {
    status: DaemonStatus,
    capabilities: Vec<CapabilityEntry>,
    component: String,
    context: Option<String>,
    policy: Option<String>,
    ticks: Vec<TickSummary>,
    plugins: Vec<PluginEntryDto>,
}

#[derive(Serialize, Deserialize)]
struct WorkerState {
    revision: u64,
    registered: bool,
    owner: Option<ClientIdentity>,
    policy_required: bool,
    approval: Option<ApprovedApplication>,
    memory: Option<ProcessMemorySource>,
    project: Option<PathBuf>,
    read: Option<ReadState>,
}

#[derive(Serialize, Deserialize)]
enum Plan {
    Read(ResponseData),
    Commit {
        request: Request,
        preparation: Option<TaskContract>,
        intent: Option<aicore_contracts::ai::intent::IntentProposal>,
    },
}

fn busy(message: impl Into<String>) -> IpcError {
    IpcError::new("ERR_RESOURCE_UNAVAILABLE", message)
}
fn mapped(error: aicore_contracts::errors::ContractError) -> IpcError {
    use aicore_contracts::errors::ContractError;
    match error {
        ContractError::NotSupported(message) => IpcError::not_supported(message),
        ContractError::PermissionDenied(message) => IpcError::new("ERR_PERMISSION_DENIED", message),
        ContractError::InvalidRequest(message) | ContractError::ContractInvalid(message) => {
            IpcError::invalid_request(message)
        },
        other => busy(other.to_string()),
    }
}
fn check_deadline(deadline: Instant) -> Result<(), IpcError> {
    if Instant::now() >= deadline {
        Err(IpcError::new(
            "ERR_TIMEOUT",
            "deadline expired; no new state commit started",
        ))
    } else {
        Ok(())
    }
}
fn action(request: &Request) -> Option<(&ApplicationId, ApplicationAction)> {
    Some(match request {
        Request::Register { application_id, .. } => (application_id, ApplicationAction::Register),
        Request::Unregister { application_id } => (application_id, ApplicationAction::Unregister),
        Request::SubmitTask { contract } => {
            (&contract.application_id, ApplicationAction::SubmitTask)
        },
        Request::DeclareCurrentTask { contract } => {
            (&contract.application_id, ApplicationAction::DeclareTask)
        },
        Request::ClearCurrentTask { application_id } => {
            (application_id, ApplicationAction::ClearTask)
        },
        Request::PrepareUpcomingTask { contract, .. } => {
            (&contract.application_id, ApplicationAction::PrepareTask)
        },
        Request::SubmitNativeTask { contract } => (
            &contract.application_id,
            ApplicationAction::SubmitNativeTask,
        ),
        Request::GetNativeTask { application_id, .. } => {
            (application_id, ApplicationAction::ReadNativeTask)
        },
        Request::CancelNativeTask { application_id, .. } => {
            (application_id, ApplicationAction::CancelNativeTask)
        },
        Request::ProposeIntent { request } => {
            (&request.application_id, ApplicationAction::ProposeIntent)
        },
        _ => return None,
    })
}

impl DaemonIpcHandler {
    pub fn with_process_worker(mut self, executable: PathBuf, project: PathBuf) -> Self {
        self.process_project = Some(project);
        self.process_worker = Some(ProcessHandlerSpec {
            process: aicore_contracts::plugin::host::PluginProcessSpec {
                plugin_id: "daemon-ipc".into(),
                artifact_path: executable.to_string_lossy().into(),
                timeout_ms: 5000,
                memory_limit_mb: 128,
            },
            args: vec!["--ipc-handler-worker".into()],
        });
        self
    }

    pub(super) fn prepare_worker_state(
        &self,
        envelope: &Envelope,
        _: ClientIdentity,
        deadline: Instant,
    ) -> Result<serde_json::Value, IpcError> {
        check_deadline(deadline)?;
        let revision = self
            .process_revision
            .try_lock()
            .map_err(|_| busy("IPC state transaction busy"))?;
        let mut state = WorkerState {
            revision: *revision,
            registered: false,
            owner: None,
            policy_required: false,
            approval: None,
            memory: None,
            project: self.process_project.clone(),
            read: None,
        };
        if let Some((id, _)) = action(&envelope.payload) {
            state.owner = self
                .app_owners
                .try_lock()
                .map_err(|_| busy("application owners busy"))?
                .get(id)
                .copied();
            state.registered = self.registry.try_get(id).map_err(mapped)?.is_some();
            if let Some((policy, resolver)) = &self.identity_policy {
                if !resolver.process_resolvable() {
                    return Err(IpcError::not_supported(
                        "artifact resolver has no isolated implementation",
                    ));
                }
                state.policy_required = true;
                state.approval = policy.try_approval(id).map_err(mapped)?;
            }
        } else if !matches!(envelope.payload, Request::Handshake { .. }) {
            let status = self
                .status
                .try_lock()
                .map_err(|_| busy("daemon status busy"))?
                .clone();
            let context = if status.last_tick.is_none() {
                self.last_context_json
                    .try_lock()
                    .map_err(|_| busy("context busy"))?
                    .clone()
            } else {
                None
            };
            let policy = if status.last_tick.is_none() {
                self.last_policy_json
                    .try_lock()
                    .map_err(|_| busy("policy busy"))?
                    .clone()
            } else {
                None
            };
            let ticks = match envelope.payload {
                Request::GetRecentTicks { limit } => self.history.try_snapshot(limit)?,
                Request::GetDashboardSnapshot => self.history.try_snapshot(10)?,
                _ => Vec::new(),
            };
            let plugins = if matches!(
                envelope.payload,
                Request::ListPlugins | Request::GetDashboardSnapshot
            ) {
                self.plugins
                    .try_list()
                    .map_err(|e| busy(e.to_string()))?
                    .iter()
                    .map(plugin_entry_to_dto)
                    .collect()
            } else {
                Vec::new()
            };
            state.read = Some(ReadState {
                status,
                context,
                policy,
                ticks,
                plugins,
                capabilities: self.capabilities.clone(),
                component: self.component.clone(),
            });
        }
        if matches!(envelope.payload, Request::PrepareUpcomingTask { .. }) {
            state.memory = Some(
                self.personal_memory
                    .as_ref()
                    .ok_or_else(|| IpcError::not_supported("personal memory unavailable"))?
                    .try_process_source(1000)
                    .map_err(mapped)?,
            );
        }
        if matches!(envelope.payload, Request::ProposeIntent { .. }) && self.intent_agent.is_some()
        {
            return Err(IpcError::not_supported("in-process custom intent agents cannot run in managed IPC; configure a process provider"));
        }
        check_deadline(deadline)?;
        serde_json::to_value(state).map_err(|e| IpcError::internal(e.to_string()))
    }

    pub(super) fn commit_worker_response(
        &self,
        envelope: &Envelope,
        identity: ClientIdentity,
        state: &serde_json::Value,
        response: serde_json::Value,
        deadline: Instant,
    ) -> Result<ResponseEnvelope, IpcError> {
        check_deadline(deadline)?;
        if matches!(envelope.payload, Request::Handshake { .. }) || response["success"] == false {
            return serde_json::from_value(response).map_err(|e| IpcError::internal(e.to_string()));
        }
        let plan: Plan = serde_json::from_value(response["prepared"].clone())
            .map_err(|e| IpcError::invalid_request(format!("invalid worker plan: {e}")))?;
        let old: WorkerState =
            serde_json::from_value(state.clone()).map_err(|e| IpcError::internal(e.to_string()))?;
        let data = match plan {
            Plan::Read(data) if action(&envelope.payload).is_none() => data,
            Plan::Commit {
                request,
                preparation,
                intent,
            } if request == envelope.payload => {
                let mut revision = self
                    .process_revision
                    .try_lock()
                    .map_err(|_| busy("IPC commit busy"))?;
                if *revision != old.revision {
                    return Err(busy("IPC state changed during preparation; retry"));
                }
                let (id, _) = action(&request)
                    .ok_or_else(|| IpcError::invalid_request("mutation plan for query"))?;
                if !identity.is_current() {
                    return Err(IpcError::new(
                        "ERR_PERMISSION_DENIED",
                        "IPC peer exited before commit",
                    ));
                }
                if self
                    .app_owners
                    .try_lock()
                    .map_err(|_| busy("owners busy"))?
                    .get(id)
                    .copied()
                    != old.owner
                    || self.registry.try_get(id).map_err(mapped)?.is_some() != old.registered
                {
                    return Err(busy("application binding changed; retry"));
                }
                if let Some((policy, _)) = &self.identity_policy {
                    if policy.try_approval(id).map_err(mapped)? != old.approval {
                        return Err(IpcError::new(
                            "ERR_PERMISSION_DENIED",
                            "application approval changed",
                        ));
                    }
                }
                check_deadline(deadline)?;
                // Even a partial failed cancellation invalidates prior snapshots.
                *revision = revision.wrapping_add(1);
                let committed =
                    self.commit_prepared(request, identity, preparation, intent, deadline);
                if Instant::now() >= deadline {
                    return Err(IpcError::new(
                        "ERR_TIMEOUT",
                        if committed.is_ok() {
                            "state write completed but response deadline expired; reconnect and query state"
                        } else {
                            "state commit did not complete before deadline; partial cancellation may be present; query state"
                        },
                    ));
                }
                committed?
            },
            _ => {
                return Err(IpcError::invalid_request(
                    "worker plan does not match request",
                ))
            },
        };
        Ok(ResponseEnvelope::ok(envelope.request_id.clone(), data))
    }

    fn revoke_bounded(&self, id: &ApplicationId, deadline: Instant) -> Result<(), IpcError> {
        self.task_queue
            .try_cancel_application(id, deadline)
            .map_err(mapped)?;
        if let Some(source) = &self.l1_source {
            source.try_clear_for(id).map_err(mapped)?;
        }
        self.registry.try_unregister(id).map_err(mapped)?;
        Ok(())
    }

    fn commit_prepared(
        &self,
        request: Request,
        identity: ClientIdentity,
        preparation: Option<TaskContract>,
        intent: Option<aicore_contracts::ai::intent::IntentProposal>,
        deadline: Instant,
    ) -> Result<ResponseData, IpcError> {
        let cancel_native = matches!(&request, Request::CancelNativeTask { .. });
        match request {
            Request::Register {
                application_id,
                name,
                version,
                capabilities,
            } => {
                let mut owners = self
                    .app_owners
                    .try_lock()
                    .map_err(|_| busy("owners busy"))?;
                if owners
                    .get(&application_id)
                    .copied()
                    .is_some_and(ClientIdentity::is_stale)
                {
                    self.revoke_bounded(&application_id, deadline)?;
                    owners.remove(&application_id);
                }
                if owners
                    .get(&application_id)
                    .is_some_and(|owner| *owner != identity)
                {
                    return Err(IpcError::new(
                        "ERR_PERMISSION_DENIED",
                        "application binding changed",
                    ));
                }
                self.registry
                    .try_register(ApplicationRegistration {
                        application_id: application_id.clone(),
                        name,
                        version,
                        capabilities,
                        registered_at_unix_secs: current_unix_ms() / 1000,
                    })
                    .map_err(mapped)?;
                owners.insert(application_id.clone(), identity);
                Ok(ResponseData::Registered {
                    application_id: application_id.0,
                })
            },
            Request::Unregister { application_id } => {
                let mut owners = self
                    .app_owners
                    .try_lock()
                    .map_err(|_| busy("owners busy"))?;
                self.revoke_bounded(&application_id, deadline)?;
                owners.remove(&application_id);
                Ok(ResponseData::Unregistered { removed: true })
            },
            Request::SubmitTask { contract } => {
                let id = self
                    .task_queue
                    .try_enqueue(QueuedTask::new(
                        contract.task_id.clone().unwrap_or_default(),
                        contract.clone(),
                        current_unix_ms(),
                    ))
                    .map_err(mapped)?;
                Ok(ResponseData::TaskSubmitted {
                    accepted: true,
                    task_id: Some(id),
                    message: format!("queued for {}", contract.application_id),
                })
            },
            Request::DeclareCurrentTask { contract } => {
                self.l1_source
                    .as_ref()
                    .ok_or_else(|| IpcError::not_supported("L1 task source unavailable"))?
                    .try_set_current(contract)
                    .map_err(mapped)?;
                Ok(ResponseData::CurrentTaskDeclared)
            },
            Request::ClearCurrentTask { application_id } => {
                let cleared = self
                    .l1_source
                    .as_ref()
                    .ok_or_else(|| IpcError::not_supported("L1 task source unavailable"))?
                    .try_clear_for(&application_id)
                    .map_err(mapped)?;
                Ok(ResponseData::CurrentTaskCleared { cleared })
            },
            Request::SubmitNativeTask { contract } => {
                let handle = self
                    .l2_provider
                    .as_ref()
                    .ok_or_else(|| IpcError::not_supported("L2 native task provider unavailable"))?
                    .try_submit(&contract)
                    .map_err(mapped)?;
                Ok(ResponseData::NativeTaskSubmitted {
                    handle_id: handle.handle_id,
                })
            },
            Request::GetNativeTask {
                application_id,
                handle_id,
            }
            | Request::CancelNativeTask {
                application_id,
                handle_id,
            } => {
                let provider = self.l2_provider.as_ref().ok_or_else(|| {
                    IpcError::not_supported("L2 native task provider unavailable")
                })?;
                let handle = NativeTaskHandle { handle_id };
                let (contract, _, state) = provider.try_task_info(&handle).map_err(mapped)?;
                if contract.application_id != application_id {
                    return Err(IpcError::new(
                        "ERR_PERMISSION_DENIED",
                        "task owner mismatch",
                    ));
                }
                if cancel_native {
                    provider.try_cancel(&handle).map_err(mapped)?;
                    Ok(ResponseData::NativeTaskCancelled)
                } else {
                    Ok(ResponseData::NativeTaskStatus { state })
                }
            },
            Request::ProposeIntent { .. } => {
                let proposal =
                    intent.ok_or_else(|| IpcError::invalid_request("missing intent result"))?;
                validate_intent_proposal(&proposal, current_unix_ms())
                    .map_err(|e| IpcError::invalid_request(e.to_string()))?;
                Ok(ResponseData::IntentProposed { proposal })
            },
            Request::PrepareUpcomingTask {
                contract,
                starts_at_unix_ms,
            } => {
                let mut prepared =
                    preparation.ok_or_else(|| IpcError::invalid_request("missing preparation"))?;
                let mut expected = contract.clone();
                expected.task = prepared.task;
                expected.deadline_unix_secs = Some(starts_at_unix_ms / 1000);
                if prepared != expected {
                    return Err(IpcError::invalid_request("worker altered task constraints"));
                }
                if prepared.application_id != contract.application_id
                    || prepared.deadline_unix_secs != Some(starts_at_unix_ms / 1000)
                    || starts_at_unix_ms < current_unix_ms().saturating_add(1000)
                {
                    return Err(IpcError::invalid_request("invalid or expired preparation"));
                }
                static SEQUENCE: std::sync::atomic::AtomicU64 =
                    std::sync::atomic::AtomicU64::new(0);
                let now = current_unix_ms();
                let id = format!(
                    "preparation-{now}-{}",
                    SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                );
                prepared.task_id = Some(id.clone());
                let workload = prepared
                    .task
                    .ok_or_else(|| IpcError::invalid_request("missing preparation workload"))?;
                let guard = ApplicationPreparationGuard {
                    identity,
                    application_id: prepared.application_id.clone(),
                    owners: self.app_owners.clone(),
                    memory: self
                        .personal_memory
                        .as_ref()
                        .ok_or_else(|| IpcError::not_supported("personal memory unavailable"))?
                        .clone(),
                    registry: self.registry.clone(),
                    policy: self.identity_policy.clone(),
                    model_targets: self.preparation_model_targets.clone(),
                    expires_at_unix_ms: starts_at_unix_ms,
                };
                let ready = starts_at_unix_ms.saturating_sub(30_000).max(now);
                let id = self
                    .task_queue
                    .try_enqueue_preparation(
                        QueuedTask::new(id, prepared, now),
                        ready,
                        Arc::new(guard),
                    )
                    .map_err(mapped)?;
                Ok(ResponseData::PreparationScheduled {
                    preparation_id: id,
                    workload,
                    ready_at_unix_ms: ready,
                })
            },
            _ => Err(IpcError::invalid_request("unexpected commit operation")),
        }
    }

    pub fn reap_stale_applications_bounded(&self) -> Result<usize, IpcError> {
        let deadline = Instant::now() + std::time::Duration::from_millis(100);
        let mut revision = self
            .process_revision
            .try_lock()
            .map_err(|_| busy("IPC state transaction busy"))?;
        let mut owners = self
            .app_owners
            .try_lock()
            .map_err(|_| busy("owners busy"))?;
        let mut stale = Vec::new();
        for (app, peer) in &*owners {
            check_deadline(deadline)?;
            if peer.is_stale() {
                stale.push(app.clone());
            }
        }
        for app in &stale {
            *revision = revision.wrapping_add(1);
            self.revoke_bounded(app, deadline)?;
            owners.remove(app);
        }
        Ok(stale.len())
    }
}

pub(super) fn worker_main() -> std::io::Result<()> {
    let frame = aicore_ipc::process_handler::read_process_request()?;
    let id = frame.envelope.request_id.clone();
    let response = if matches!(frame.envelope.payload, Request::Handshake { .. }) {
        struct Handshake;
        impl IpcHandler for Handshake {
            fn handle(&self, _: Request) -> Result<ResponseData, IpcError> {
                Err(IpcError::not_supported("handshake only"))
            }
            fn name(&self) -> &'static str {
                "handshake"
            }
        }
        serde_json::to_value(aicore_ipc::server::process_envelope(
            &(Arc::new(Handshake) as Arc<dyn IpcHandler>),
            frame.envelope,
            frame.identity,
        ))?
    } else {
        match prepare_in_worker(&frame) {
            Ok(plan) => {
                serde_json::json!({"version":frame.envelope.version,"request_id":id,"success":true,"prepared":plan})
            },
            Err(error) => {
                let mut response = ResponseEnvelope::err(id, error);
                response.version = frame.envelope.version;
                serde_json::to_value(response)?
            },
        }
    };
    aicore_ipc::process_handler::write_process_response(&response)
}

fn memory_input(
    source: ProcessMemorySource,
) -> Result<(UserProfile, Vec<HistoricalRecord>), IpcError> {
    match source {
        ProcessMemorySource::Snapshot { profile, records } => Ok((profile, records)),
        ProcessMemorySource::Sqlite { path } => {
            let store = aicore_memory::SqliteMemoryStore::open_read_only(path).map_err(mapped)?;
            Ok((
                store.load_profile().map_err(mapped)?,
                store.load_records(1000).map_err(mapped)?,
            ))
        },
    }
}

fn prepare_in_worker(frame: &ProcessHandlerRequest) -> Result<Plan, IpcError> {
    let state: WorkerState = serde_json::from_value(frame.state.clone())
        .map_err(|e| IpcError::invalid_request(e.to_string()))?;
    let request = &frame.envelope.payload;
    let Some((id, operation)) = action(request) else {
        return query_plan(
            state
                .read
                .ok_or_else(|| IpcError::internal("missing read snapshot"))?,
            request,
        );
    };
    if !frame.identity.is_current() {
        return Err(IpcError::new(
            "ERR_PERMISSION_DENIED",
            "IPC peer is no longer current",
        ));
    }
    if state.policy_required {
        let policy = ApplicationIdentityPolicy::new();
        if let Some(approval) = state.approval {
            policy.pin(approval).map_err(mapped)?;
        }
        let digest = frame.identity.artifact_sha256().ok_or_else(|| {
            IpcError::new(
                "ERR_PERMISSION_DENIED",
                "application artifact identity unavailable",
            )
        })?;
        policy
            .authorize_identity(
                id,
                &digest,
                frame.identity.principal().as_deref(),
                operation,
            )
            .map_err(mapped)?;
    }
    if operation == ApplicationAction::Register {
        if state
            .owner
            .is_some_and(|owner| !owner.is_stale() && owner != frame.identity)
            || (state.owner.is_none() && state.registered)
        {
            return Err(IpcError::new(
                "ERR_PERMISSION_DENIED",
                "application already registered by another peer",
            ));
        }
        if !id.is_valid() {
            return Err(IpcError::invalid_request("invalid application_id"));
        }
    } else if state.owner != Some(frame.identity) {
        return Err(IpcError::new(
            "ERR_PERMISSION_DENIED",
            "application is not bound to this IPC peer",
        ));
    }
    let contract = match request {
        Request::SubmitTask { contract }
        | Request::DeclareCurrentTask { contract }
        | Request::PrepareUpcomingTask { contract, .. }
        | Request::SubmitNativeTask { contract } => Some(contract),
        _ => None,
    };
    if let Some(contract) = contract {
        if !state.registered {
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
                "task_id must not be empty or exceed 256 chars",
            ));
        }
        if contract.deadline_unix_secs == Some(0) {
            return Err(IpcError::invalid_request("deadline_unix_secs must be > 0"));
        }
    }
    let mut preparation = None;
    let mut intent = None;
    if let Request::PrepareUpcomingTask {
        contract,
        starts_at_unix_ms,
    } = request
    {
        let (profile, records) = memory_input(
            state
                .memory
                .ok_or_else(|| IpcError::not_supported("personal memory unavailable"))?,
        )?;
        if !profile.can_personalize() {
            return Err(IpcError::new(
                "ERR_PERMISSION_DENIED",
                "personalization consent required",
            ));
        }
        let now = current_unix_ms();
        if *starts_at_unix_ms < now.saturating_add(1000)
            || *starts_at_unix_ms > now.saturating_add(300_000)
        {
            return Err(IpcError::invalid_request(
                "upcoming task must start within 1..300 seconds",
            ));
        }
        let cutoff =
            now.saturating_sub(u64::from(profile.retention_days).saturating_mul(86_400_000));
        let records: Vec<_> = records
            .into_iter()
            .filter(|r| r.timestamp_unix_ms >= cutoff && r.timestamp_unix_ms <= now)
            .collect();
        let aggregate = aicore_contracts::memory::context::MemoryContext::from_records(&records);
        let hour = ((*starts_at_unix_ms / 3_600_000) % 24) as u8;
        let pattern = aggregate
            .application_patterns
            .iter()
            .find(|p| {
                &p.application_id == id
                    && p.hour_utc == hour
                    && p.sample_count >= 3
                    && p.confidence >= 0.8
            })
            .ok_or_else(|| {
                IpcError::not_supported("no reliable application pattern for upcoming time")
            })?;
        if contract
            .task
            .is_some_and(|w| w != pattern.dominant_workload)
        {
            return Err(IpcError::invalid_request(
                "explicit task differs from personal prediction",
            ));
        }
        let mut contract = contract.clone();
        contract.task = Some(pattern.dominant_workload);
        contract.deadline_unix_secs = Some(*starts_at_unix_ms / 1000);
        preparation = Some(contract);
    }
    if let Request::ProposeIntent { request } = request {
        if request.text.trim().is_empty() || request.text.len() > 4096 {
            return Err(IpcError::invalid_request("invalid intent text"));
        }
        let project = state
            .project
            .ok_or_else(|| IpcError::not_supported("local intent provider unavailable"))?;
        let agent = aicore_ai::configured_intent_agent(&project)
            .map_err(|e| IpcError::new("ERR_AI_INFERENCE_FAILED", e.to_string()))?
            .ok_or_else(|| IpcError::not_supported("local intent provider unavailable"))?;
        intent = Some(
            aicore_contracts::traits::intent_agent::IIntentAgent::propose(agent.as_ref(), request)
                .map_err(|e| IpcError::new("ERR_AI_INFERENCE_FAILED", e.to_string()))?,
        );
    }
    Ok(Plan::Commit {
        request: request.clone(),
        preparation,
        intent,
    })
}

fn query_plan(state: ReadState, request: &Request) -> Result<Plan, IpcError> {
    let context = state
        .status
        .last_tick
        .as_ref()
        .map(|t| &t.current_context_json)
        .unwrap_or(&state.context)
        .as_ref()
        .and_then(|j| serde_json::from_str(j).ok());
    let policy = state
        .status
        .last_tick
        .as_ref()
        .map(|t| &t.current_policy_json)
        .unwrap_or(&state.policy)
        .as_ref()
        .and_then(|j| serde_json::from_str(j).ok());
    let data = match request {
        Request::GetStatus => ResponseData::Status {
            status: state.status,
        },
        Request::GetCapabilities => ResponseData::Capabilities {
            component: state.component,
            entries: state.capabilities,
        },
        Request::GetContext => ResponseData::Context { context },
        Request::GetPolicy => ResponseData::Policy { policy },
        Request::GetRecentTicks { .. } => ResponseData::RecentTicks { ticks: state.ticks },
        Request::ListPlugins => ResponseData::Plugins {
            entries: state.plugins,
        },
        Request::GetDashboardSnapshot => {
            let mut snapshot = DashboardSnapshot::new(current_unix_ms());
            if let Some(tick) = &state.status.last_tick {
                snapshot.current_workload = Some(tick.workload_type.clone());
                snapshot.runtime_route = tick
                    .runtime_route_json
                    .as_ref()
                    .and_then(|j| serde_json::from_str(j).ok());
                snapshot.ai_runtime_health = tick
                    .ai_runtime_health_json
                    .as_ref()
                    .and_then(|j| serde_json::from_str(j).ok());
                snapshot.prediction = tick.predicted_workload.map(|w| {
                    aicore_contracts::dashboard::snapshot::PredictionSummary {
                        predicted_workload: format!("{:?}", w),
                        confidence: tick.prediction_confidence.unwrap_or(0.0),
                        horizon_ms: None,
                    }
                });
                snapshot.execution_status = tick.execution_status.map(|s| {
                    aicore_contracts::dashboard::snapshot::ExecutionSummary {
                        policy_id: tick.policy_id.clone().unwrap_or_default(),
                        overall_status: format!("{:?}", s),
                        outcomes_count: 0,
                        duration_ms: tick.duration_ms,
                    }
                });
            }
            snapshot.current_context = context;
            snapshot.current_policy = policy;
            snapshot.daemon_status = Some(state.status);
            snapshot.capabilities = state.capabilities;
            snapshot.plugin_states = state.plugins;
            snapshot.recent_ticks = state.ticks;
            ResponseData::DashboardSnapshot {
                snapshot: Box::new(snapshot),
            }
        },
        _ => return Err(IpcError::invalid_request("unexpected query")),
    };
    Ok(Plan::Read(data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use aicore_task_runtime::InMemoryTaskQueue;
    use std::time::Duration;

    fn handler() -> (DaemonIpcHandler, Arc<InMemoryTaskQueue>) {
        let queue = Arc::new(InMemoryTaskQueue::new());
        let provider = Arc::new(L2NativeTaskProvider::new().with_task_queue(queue.clone()));
        (
            DaemonIpcHandler::new(
                Arc::new(ApplicationRegistry::new()),
                Arc::new(Mutex::new(DaemonStatus::new(1))),
                Arc::new(TickHistory::with_default_capacity()),
                Arc::new(PluginRegistry::new()),
                queue.clone(),
                Some(Arc::new(L1TaskSource::new())),
                Some(provider),
                vec![],
                "broker-test".into(),
                Arc::new(Mutex::new(None)),
                Arc::new(Mutex::new(None)),
            ),
            queue,
        )
    }
    fn register() -> Request {
        Request::Register {
            application_id: ApplicationId("broker.app".into()),
            name: "test".into(),
            version: "1".into(),
            capabilities: vec![],
        }
    }
    fn prepare(
        handler: &DaemonIpcHandler,
        request: Request,
    ) -> (Envelope, serde_json::Value, serde_json::Value) {
        let envelope = Envelope::new("broker", request);
        let state = handler
            .prepare_worker_state(
                &envelope,
                ClientIdentity::Internal,
                Instant::now() + Duration::from_secs(1),
            )
            .unwrap();
        let plan = prepare_in_worker(&ProcessHandlerRequest {
            envelope: envelope.clone(),
            state: state.clone(),
            identity: ClientIdentity::Internal,
        })
        .unwrap();
        let response = serde_json::json!({"success":true,"prepared":plan});
        (envelope, state, response)
    }
    fn commit(
        handler: &DaemonIpcHandler,
        prepared: (Envelope, serde_json::Value, serde_json::Value),
    ) -> Result<ResponseEnvelope, IpcError> {
        handler.commit_worker_response(
            &prepared.0,
            ClientIdentity::Internal,
            &prepared.1,
            prepared.2,
            Instant::now() + Duration::from_secs(1),
        )
    }
    #[test]
    fn worker_preparation_has_no_effect_until_acknowledged_commit() {
        let (handler, queue) = handler();
        let prepared = prepare(&handler, register());
        assert_eq!(handler.registry.count().unwrap(), 0);
        assert!(commit(&handler, prepared).unwrap().success);
        let mut contract = TaskContract::minimal(ApplicationId("broker.app".into()));
        contract.task_id = Some("explicit-broker-task".into());
        let prepared = prepare(&handler, Request::SubmitTask { contract });
        assert!(queue.snapshot().unwrap().is_empty());
        let response = commit(&handler, prepared).unwrap();
        assert!(
            matches!(response.data,Some(ResponseData::TaskSubmitted { task_id:Some(id),accepted:true,.. }) if id == "explicit-broker-task")
        );
        assert_eq!(
            queue
                .get("explicit-broker-task")
                .unwrap()
                .unwrap()
                .contract
                .application_id
                .0,
            "broker.app"
        );
    }
    #[test]
    fn concurrent_stale_plan_cannot_commit_or_repeat_a_write() {
        let (handler, _) = handler();
        let first = prepare(&handler, register());
        let stale = prepare(&handler, register());
        commit(&handler, first).unwrap();
        assert_eq!(
            commit(&handler, stale).unwrap_err().code,
            "ERR_RESOURCE_UNAVAILABLE"
        );
        assert_eq!(handler.registry.count().unwrap(), 1);
    }
    #[test]
    fn expired_commit_does_not_write_and_busy_state_never_waits() {
        let (handler, _) = handler();
        let prepared = prepare(&handler, register());
        assert_eq!(
            handler
                .commit_worker_response(
                    &prepared.0,
                    ClientIdentity::Internal,
                    &prepared.1,
                    prepared.2,
                    Instant::now()
                )
                .unwrap_err()
                .code,
            "ERR_TIMEOUT"
        );
        assert_eq!(handler.registry.count().unwrap(), 0);
        let _locked = handler.status.lock().unwrap();
        let now = Instant::now();
        assert_eq!(
            handler
                .prepare_worker_state(
                    &Envelope::new("busy", Request::GetStatus),
                    ClientIdentity::Internal,
                    now + Duration::from_secs(1)
                )
                .unwrap_err()
                .code,
            "ERR_RESOURCE_UNAVAILABLE"
        );
        assert!(now.elapsed() < Duration::from_millis(100));
    }
    #[test]
    fn native_cancel_and_unregister_change_the_original_shared_queue() {
        use aicore_contracts::task::lifecycle::TaskState;
        let (handler, queue) = handler();
        commit(&handler, prepare(&handler, register())).unwrap();
        let contract = TaskContract::minimal(ApplicationId("broker.app".into()));
        let response = commit(
            &handler,
            prepare(
                &handler,
                Request::SubmitNativeTask {
                    contract: contract.clone(),
                },
            ),
        )
        .unwrap();
        let Some(ResponseData::NativeTaskSubmitted { handle_id }) = response.data else {
            panic!()
        };
        assert_eq!(
            queue.get(&handle_id).unwrap().unwrap().state,
            TaskState::Queued
        );
        commit(
            &handler,
            prepare(
                &handler,
                Request::CancelNativeTask {
                    application_id: contract.application_id.clone(),
                    handle_id: handle_id.clone(),
                },
            ),
        )
        .unwrap();
        assert_eq!(
            queue.get(&handle_id).unwrap().unwrap().state,
            TaskState::Cancelled
        );
        let pending = commit(
            &handler,
            prepare(
                &handler,
                Request::SubmitTask {
                    contract: contract.clone(),
                },
            ),
        )
        .unwrap();
        let Some(ResponseData::TaskSubmitted {
            task_id: Some(id), ..
        }) = pending.data
        else {
            panic!()
        };
        commit(
            &handler,
            prepare(
                &handler,
                Request::Unregister {
                    application_id: contract.application_id,
                },
            ),
        )
        .unwrap();
        assert_eq!(queue.get(&id).unwrap().unwrap().state, TaskState::Cancelled);
        assert_eq!(handler.registry.count().unwrap(), 0);
    }
    #[test]
    fn changed_owner_and_altered_contract_are_rejected_before_write() {
        let (handler, queue) = handler();
        commit(&handler, prepare(&handler, register())).unwrap();
        let mut contract = TaskContract::minimal(ApplicationId("broker.app".into()));
        contract.task_id = Some("unchanged".into());
        let mut prepared = prepare(&handler, Request::SubmitTask { contract });
        prepared.2["prepared"]["Commit"]["request"]["contract"]["task_id"] =
            serde_json::json!("tampered");
        assert_eq!(
            commit(&handler, prepared).unwrap_err().code,
            "ERR_INVALID_REQUEST"
        );
        assert!(queue.snapshot().unwrap().is_empty());
        let prepared = prepare(
            &handler,
            Request::Unregister {
                application_id: ApplicationId("broker.app".into()),
            },
        );
        handler.app_owners.lock().unwrap().clear();
        assert_eq!(
            commit(&handler, prepared).unwrap_err().code,
            "ERR_RESOURCE_UNAVAILABLE"
        );
        assert_eq!(handler.registry.count().unwrap(), 1);
    }
    #[test]
    fn non_reopenable_memory_and_unknown_resolver_are_refused() {
        let memory = aicore_memory::SqliteMemoryStore::open_in_memory().unwrap();
        assert!(matches!(
            memory.try_process_source(1),
            Err(aicore_contracts::errors::ContractError::NotSupported(_))
        ));
        struct Unknown;
        impl IApplicationArtifactResolver for Unknown {
            fn artifact_sha256(&self, _: ClientIdentity) -> Option<String> {
                panic!("must not call synchronous resolver")
            }
        }
        let (handler, _) = handler();
        let handler = handler.with_identity_policy(
            Arc::new(ApplicationIdentityPolicy::new()),
            Arc::new(Unknown),
        );
        assert_eq!(
            handler
                .prepare_worker_state(
                    &Envelope::new("refused", register()),
                    ClientIdentity::Internal,
                    Instant::now() + Duration::from_secs(1)
                )
                .unwrap_err()
                .code,
            "ERR_NOT_SUPPORTED"
        );
    }
}
