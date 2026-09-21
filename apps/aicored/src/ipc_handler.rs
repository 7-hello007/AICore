use std::sync::{Arc, Mutex};

use aicore_api::ApplicationRegistry;
use aicore_contracts::dashboard::snapshot::DashboardSnapshot;
use aicore_contracts::task::queued::QueuedTask;
use aicore_contracts::traits::task_queue::ITaskQueue;
use aicore_contracts::{
    application::id::ApplicationId,
    capability::entry::CapabilityEntry,
    ipc::protocol::{IpcError, PluginEntryDto, Request, ResponseData},
    runtime::daemon_status::DaemonStatus,
};
use aicore_ipc::IpcHandler;
use aicore_plugin::PluginRegistry;

use crate::tick_history::TickHistory;

/// Daemon 的 IPC 请求处理器。
pub struct DaemonIpcHandler {
    registry: Arc<ApplicationRegistry>,
    status: Arc<Mutex<DaemonStatus>>,
    history: Arc<TickHistory>,
    plugins: Arc<PluginRegistry>,
    task_queue: Arc<dyn ITaskQueue>,
    capabilities: Vec<CapabilityEntry>,
    component: String,
    last_policy_json: Arc<Mutex<Option<String>>>,
    last_context_json: Arc<Mutex<Option<String>>>,
}

impl DaemonIpcHandler {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        registry: Arc<ApplicationRegistry>,
        status: Arc<Mutex<DaemonStatus>>,
        history: Arc<TickHistory>,
        plugins: Arc<PluginRegistry>,
        task_queue: Arc<dyn ITaskQueue>,
        capabilities: Vec<CapabilityEntry>,
        component: String,
        last_policy_json: Arc<Mutex<Option<String>>>,
        last_context_json: Arc<Mutex<Option<String>>>,
    ) -> Self {
        Self {
            registry,
            status,
            history,
            plugins,
            task_queue,
            capabilities,
            component,
            last_policy_json,
            last_context_json,
        }
    }
}

impl IpcHandler for DaemonIpcHandler {
    fn handle(&self, request: Request) -> Result<ResponseData, IpcError> {
        match request {
            Request::Register {
                application_id,
                name,
                version,
                capabilities,
            } => {
                let id = ApplicationId::new(application_id.0.clone());
                match self
                    .registry
                    .register(id.clone(), name, version, capabilities)
                {
                    Ok(_) => Ok(ResponseData::Registered {
                        application_id: id.0,
                    }),
                    Err(e) => Err(IpcError::invalid_request(format!("{}", e))),
                }
            },

            Request::Unregister { application_id } => {
                let id = ApplicationId::new(application_id.0.clone());
                match self.registry.unregister(&id) {
                    Ok(removed) => Ok(ResponseData::Unregistered { removed }),
                    Err(e) => Err(IpcError::internal(format!("{}", e))),
                }
            },

            Request::SubmitTask { contract } => {
                let registered = self
                    .registry
                    .is_registered(&contract.application_id)
                    .unwrap_or(false);
                if !registered {
                    return Err(IpcError::new(
                        "ERR_APP_NOT_REGISTERED",
                        format!("application not registered: {}", contract.application_id),
                    ));
                }
                if let Some(tid) = &contract.task_id {
                    if tid.trim().is_empty() || tid.len() > 256 {
                        return Err(IpcError::invalid_request(
                            "task_id must be non-empty and <= 256 chars",
                        ));
                    }
                }
                if let Some(d) = contract.deadline_unix_secs {
                    if d == 0 {
                        return Err(IpcError::invalid_request("deadline_unix_secs must be > 0"));
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
            .map(|d| d.as_secs())
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
