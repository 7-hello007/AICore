use std::path::PathBuf;
use std::sync::{mpsc as sync_mpsc, Arc, Mutex};
use std::time::Duration;

use aicore_ai::configured_intent_agent;
use aicore_api::{ApplicationIdentityPolicy, ApplicationRegistry, ApprovedApplication};
use aicore_contracts::{
    capability::entry::CapabilityEntry,
    runtime::{daemon_config::DaemonConfig, daemon_status::DaemonStatus, tick::TickOutcome},
    traits::runtime_orchestrator::IRuntimeOrchestrator,
};
use aicore_ipc::{IpcHandler, IpcServer};
use aicore_plugin::{
    discover_from_dir, register_discovered, ManagedPluginProcessController,
    ManagedPluginTelemetryProvider, PluginActivationManager, PluginActivationPolicy,
    PluginRegistry, SubprocessPluginHost,
};
use aicore_runtime::dual_loop::{ModelTickDriver, PlatformTickDriver};
use tokio::sync::mpsc;

use crate::ipc_handler::{DaemonIpcHandler, PeerExecutableArtifactResolver};
use crate::runtime::DaemonRuntime;
use crate::state::write_status_atomic;
use crate::tick_history::TickHistory;

pub struct SchedulerConfig {
    pub runtime: DaemonRuntime,
    pub config: DaemonConfig,
    pub state_path: PathBuf,
    pub ipc_path: PathBuf,
    pub plugins_dir: PathBuf,
    pub plugin_policy: Option<PathBuf>,
    pub task_queue: Arc<dyn aicore_contracts::traits::task_queue::ITaskQueue>,
    pub once: bool,
    pub shutdown_rx: mpsc::Receiver<()>,
}

/// Runs the daemon's main loop.
pub async fn run(params: SchedulerConfig) -> std::io::Result<()> {
    let SchedulerConfig {
        runtime,
        config,
        state_path,
        ipc_path,
        plugins_dir,
        plugin_policy,
        task_queue,
        once,
        mut shutdown_rx,
    } = params;
    let mut runtime = runtime;

    let pid = std::process::id();
    let status = Arc::new(Mutex::new(DaemonStatus::new(pid)));
    let history = Arc::new(TickHistory::with_default_capacity());
    let plugins = Arc::new(PluginRegistry::new());

    let report = discover_from_dir(&plugins_dir);
    match register_discovered(&plugins, &report) {
        Ok(n) => tracing::info!(
            component = "aicored",
            event = "plugins_registered",
            registered = n,
            discovered = report.discovered.len(),
            rejected = report.rejected.len(),
            dir = %plugins_dir.display(),
            "plugin discovery complete"
        ),
        Err(e) => tracing::warn!(
            component = "aicored",
            event = "plugin_register_failed",
            error = %e,
            "plugin registration failed"
        ),
    }

    let activation = Arc::new(PluginActivationManager::new(
        plugins.clone(),
        Arc::new(SubprocessPluginHost::new()),
    ));
    let policy = if runtime.model_mode || runtime.simulated_platform {
        if plugin_policy.is_some() {
            tracing::warn!(
                component = "aicored",
                event = "isolated_plugin_activation_disabled",
                "Model Mode and simulated Platform Mode never start external plugin processes"
            );
        }
        None
    } else {
        plugin_policy
            .as_ref()
            .and_then(|path| match read_activation_policy(path, &plugins_dir) {
                Ok(policy) => Some(policy),
                Err(error) => {
                    tracing::warn!(component="aicored", event="plugin_policy_rejected", error=%error);
                    None
                },
            })
    };
    for discovered in &report.discovered {
        let pins: Vec<_> = policy
            .as_ref()
            .map(|policy| {
                policy
                    .pins
                    .iter()
                    .filter(|pin| pin.plugin_id == discovered.manifest.name)
                    .collect()
            })
            .unwrap_or_default();
        let result = if pins.len() == 1 {
            activation.activate(discovered, pins[0])
        } else {
            activation.reject_unpinned(discovered)
        };
        if let Err(error) = result {
            tracing::warn!(component="aicored", event="plugin_activation_rejected", plugin=%discovered.manifest.name, error=%error);
        }
    }
    if !runtime.model_mode && !runtime.simulated_platform {
        match activation.bound_telemetry_ids() {
            Ok(ids) if ids.len() == 1 => {
                runtime
                    .telemetry_selection
                    .bind(Arc::new(ManagedPluginTelemetryProvider::new(
                        activation.clone(),
                        ids[0].clone(),
                    )))
            },
            Ok(ids) if ids.len() > 1 => tracing::warn!(
                component = "aicored",
                event = "plugin_telemetry_ambiguous",
                count = ids.len(),
                "multiple telemetry plugins; platform source retained"
            ),
            Ok(_) => {},
            Err(error) => {
                tracing::warn!(component="aicored", event="plugin_telemetry_unavailable", error=%error)
            },
        }
        match activation.bound_process_priority_ids() {
            Ok(ids) if ids.len() == 1 => {
                if let Err(error) =
                    runtime
                        .process_selection
                        .bind(Arc::new(ManagedPluginProcessController::new(
                            activation.clone(),
                            ids[0].clone(),
                        )))
                {
                    tracing::warn!(component="aicored", event="plugin_process_control_bind_failed", error=%error);
                } else if let Some(entry) = runtime
                    .capabilities
                    .iter_mut()
                    .find(|entry| entry.name == "process.affinity.write")
                {
                    *entry = CapabilityEntry::unsupported(
                        "process.affinity.write",
                        "selected typed process plugin binds priority only",
                    );
                }
            },
            Ok(ids) if ids.len() > 1 => tracing::warn!(
                component = "aicored",
                event = "plugin_process_control_ambiguous",
                count = ids.len(),
                "multiple process controllers; no plugin control bound"
            ),
            Ok(_) => {},
            Err(error) => tracing::warn!(
                component = "aicored",
                event = "plugin_process_control_unavailable",
                error = %error
            ),
        }
    }

    let last_policy_json: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let last_context_json: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let targets = runtime.execution_targets.clone();

    {
        let guard = status.lock().expect("status lock");
        write_status_atomic(&state_path, &guard)?;
    }

    let interval_ms = config.telemetry.sampling_interval_ms;
    let write_every = config.state_write_interval_ticks;
    let model_driver = if runtime.model_mode && !once {
        Some(ModelTickDriver::new(
            runtime.loop_.clone(),
            targets.clone(),
            Duration::from_millis(interval_ms),
        )?)
    } else {
        None
    };
    let platform_driver = if !runtime.model_mode && !once {
        Some(PlatformTickDriver::new(
            runtime.loop_.clone(),
            targets.clone(),
            Duration::from_millis(interval_ms),
        )?)
    } else {
        None
    };
    let retention_sender = if let (true, Some(retention)) = (!once, &runtime.retention) {
        let (sender, receiver) = sync_mpsc::sync_channel::<u64>(1);
        let retention = retention.clone();
        std::thread::Builder::new()
            .name("aicore-model-retention".into())
            .spawn(move || {
                while let Ok(now) = receiver.recv() {
                    run_retention(&retention, now);
                }
            })?;
        Some(sender)
    } else {
        None
    };

    tracing::info!(
        component = "aicored",
        event = "daemon_started",
        pid = pid,
        platform = runtime.platform_name,
        interval_ms = interval_ms,
        state_path = %state_path.display(),
        ipc_path = %ipc_path.display(),
        plugins_dir = %plugins_dir.display(),
        declared_hardware_devices = runtime.hardware_matrices.len(),
        "daemon started"
    );

    let registry = Arc::new(ApplicationRegistry::new());
    let mut handler = DaemonIpcHandler::new(
        registry,
        status.clone(),
        history.clone(),
        plugins.clone(),
        task_queue.clone(),
        runtime.l1_source.clone(),
        runtime.l2_provider.clone(),
        runtime.capabilities.clone(),
        format!("{}-daemon", runtime.platform_name),
        last_policy_json.clone(),
        last_context_json.clone(),
    );
    if let Some(path) = std::env::var_os("AICORE_APP_POLICY_PATH") {
        let path = PathBuf::from(path);
        if !path.is_absolute() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "application policy path must be absolute",
            ));
        }
        let pins: Vec<ApprovedApplication> = serde_json::from_slice(&std::fs::read(path)?)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        let policy = Arc::new(ApplicationIdentityPolicy::new());
        for pin in pins {
            policy.pin(pin).map_err(|error| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, error.to_string())
            })?;
        }
        handler = handler.with_identity_policy(policy, Arc::new(PeerExecutableArtifactResolver));
    }
    if let Some(memory) = &runtime.memory_store {
        let model_targets = if runtime.model_mode || runtime.simulated_platform {
            Some(runtime.execution_targets.clone())
        } else {
            None
        };
        handler = handler.with_personal_memory(memory.clone(), model_targets);
    }
    let project = std::env::var_os("AICORE_PROJECT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."));
    if let Some(agent) =
        configured_intent_agent(&project).map_err(|e| std::io::Error::other(e.to_string()))?
    {
        handler = handler.with_intent_agent(agent);
    }
    let handler = Arc::new(handler);

    let stale_reaper = if once {
        None
    } else {
        let handler = handler.clone();
        Some(tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                let handler = handler.clone();
                match tokio::task::spawn_blocking(move || handler.reap_stale_applications()).await {
                    Ok(Ok(0)) => {},
                    Ok(Ok(count)) => {
                        tracing::info!(component = "aicored", event = "stale_apps_reaped", count)
                    },
                    Ok(Err(error)) => {
                        tracing::warn!(component="aicored", event="stale_app_reap_failed", error=%error.message)
                    },
                    Err(error) => {
                        tracing::warn!(component="aicored", event="stale_app_reap_join_failed", error=%error)
                    },
                }
            }
        }))
    };
    let handler: Arc<dyn IpcHandler> = handler;

    let (ipc_shutdown_tx, ipc_shutdown_rx) = mpsc::channel::<()>(1);
    let ipc_server = IpcServer::bind(ipc_path.clone(), handler)?.with_shutdown(ipc_shutdown_rx);

    let ipc_task = if once {
        None
    } else {
        let ipc = ipc_server;
        Some(tokio::spawn(async move {
            ipc.run().await;
        }))
    };

    if once {
        let outcome = runtime.loop_.tick(&targets);
        {
            let mut guard = status.lock().expect("status lock");
            guard.record_tick(&outcome);
            write_status_atomic(&state_path, &guard)?;
        }
        history.append(&outcome);
        log_tick(&outcome);
        tracing::info!(
            component = "aicored",
            event = "daemon_once_done",
            tick_id = outcome.tick_id,
            success = outcome.success,
            "once mode finished"
        );
        return Ok(());
    }

    let mut interval = tokio::time::interval(Duration::from_millis(interval_ms));
    interval.tick().await;

    loop {
        tokio::select! {
            biased;

            _ = shutdown_rx.recv() => {
                tracing::info!(
                    component = "aicored",
                    event = "shutdown_signal",
                    "shutdown signal received"
                );
                break;
            }

            _ = interval.tick() => {
                activation.refresh_health();
                let outcome = if let Some(driver) = &model_driver {
                    driver.tick()
                } else if let Some(driver) = &platform_driver {
                    driver.tick()
                } else {
                    runtime.loop_.tick(&targets)
                };
                let count = {
                    let mut guard = status.lock().expect("status lock");
                    guard.record_tick(&outcome);
                    guard.tick_count
                };
                history.append(&outcome);
                log_tick(&outcome);

                // Retention may touch disk; continuous modes keep it outside the fast tick.
                if let Some(retention) = &runtime.retention {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis().min(u64::MAX as u128) as u64)
                        .unwrap_or(0);
                    if let Some(sender) = &retention_sender {
                        let _ = sender.try_send(now);
                    } else {
                        run_retention(retention, now);
                    }
                }

                if count % write_every == 0 {
                    let guard = status.lock().expect("status lock");
                    if let Err(e) = write_status_atomic(&state_path, &guard) {
                        tracing::warn!(
                            component = "aicored",
                            event = "state_write_failed",
                            error = %e,
                            "failed to write state file"
                        );
                    }
                }
            }
        }
    }

    let _ = ipc_shutdown_tx.send(()).await;
    if let Some(task) = stale_reaper {
        task.abort();
    }
    if let Some(task) = ipc_task {
        let _ = tokio::time::timeout(Duration::from_millis(config.shutdown_timeout_ms), task).await;
    }

    let metrics = runtime.loop_.metrics();
    tracing::info!(
        component = "aicored",
        event = "daemon_stopping",
        successful = metrics.successful_ticks,
        failed = metrics.failed_ticks,
        current_mode = ?runtime.loop_.current_mode(),
        "daemon stopping"
    );

    let guard = status.lock().expect("status lock");
    let _ = write_status_atomic(&state_path, &guard);
    Ok(())
}

fn run_retention(retention: &aicore_memory::RetentionScheduler, now_unix_ms: u64) {
    if !retention.should_run(now_unix_ms) {
        return;
    }
    match retention.run_once_at(now_unix_ms) {
        Ok(report) => tracing::info!(
            component = "aicored",
            event = "retention_ran",
            removed = report.records_removed,
            retention_days = report.retention_days,
            skipped = report.skipped,
            success = report.success,
            "retention ran"
        ),
        Err(error) => tracing::warn!(
            component = "aicored",
            event = "retention_failed",
            error = %error,
            "retention failed"
        ),
    }
}

fn read_activation_policy(
    path: &std::path::Path,
    plugins_dir: &std::path::Path,
) -> std::io::Result<PluginActivationPolicy> {
    let policy = std::fs::canonicalize(path)?;
    if let Ok(discovery_dir) = std::fs::canonicalize(plugins_dir) {
        if policy.starts_with(discovery_dir) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "plugin policy must be outside discovery directory",
            ));
        }
    }
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "plugin policy must be a regular file",
        ));
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::MetadataExt;
        let daemon_uid = std::fs::metadata("/proc/self")?.uid();
        if metadata.uid() != daemon_uid || metadata.mode() & 0o022 != 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "plugin policy must be owned by daemon UID and not group/world writable",
            ));
        }
    }
    let bytes = std::fs::read(policy)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}

fn log_tick(outcome: &TickOutcome) {
    tracing::info!(
        component = "aicored",
        event = "tick",
        tick_id = outcome.tick_id,
        trace_id = %outcome.trace_id,
        success = outcome.success,
        duration_ms = outcome.duration_ms,
        mode = ?outcome.mode_after,
        workload = ?outcome.workload_type,
        policy_id = ?outcome.policy_id,
        predicted = ?outcome.predicted_workload,
        prediction_conf = ?outcome.prediction_confidence,
        "tick completed"
    );
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use aicore_task_runtime::InMemoryTaskQueue;

    #[tokio::test]
    async fn model_daemon_uses_bounded_slow_worker_and_shuts_down() {
        let root = std::env::temp_dir().join(format!(
            "aicore-model-daemon-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let queue: Arc<dyn aicore_contracts::traits::task_queue::ITaskQueue> =
            Arc::new(InMemoryTaskQueue::new());
        let runtime = crate::runtime::build_runtime_for_mode(None, Some(queue.clone()), true);
        assert!(runtime.model_mode);
        assert!(runtime.hardware_matrices.is_empty());
        let mut config = DaemonConfig::default();
        config.telemetry.sampling_interval_ms = 100;
        config.state_write_interval_ticks = 1;
        let state_path = root.join("state.json");
        let (sender, receiver) = mpsc::channel(1);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(800)).await;
            let _ = sender.send(()).await;
        });
        run(SchedulerConfig {
            runtime,
            config,
            state_path: state_path.clone(),
            ipc_path: root.join("daemon.sock"),
            plugins_dir: root.join("plugins"),
            plugin_policy: None,
            task_queue: queue,
            once: false,
            shutdown_rx: receiver,
        })
        .await
        .expect("isolated model daemon");
        let status: DaemonStatus =
            serde_json::from_slice(&std::fs::read(&state_path).expect("model state file"))
                .expect("valid model status");
        assert!(status.tick_count >= 2);
        assert!(status.last_tick.is_some());
        assert!(
            status.successful_ticks > 0,
            "a prepared model policy should be applied"
        );
    }
}
