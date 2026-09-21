mod cli;
mod ipc_handler;
mod lifecycle;
mod runtime;
mod scheduler;
mod state;
mod tick_history;

use std::path::PathBuf;
use std::process::ExitCode;

use aicore_contracts::config::loader::{apply_env_overrides, default_config_path, load_from_file};
use aicore_contracts::config::AicoreConfig;
use aicore_contracts::traits::task_queue::ITaskQueue;
use aicore_task_runtime::InMemoryTaskQueue;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::cli::parse_args;
use crate::state::default_state_path;

fn default_ipc_path() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        std::env::temp_dir().join("aicore").join("aicored.sock")
    }
    #[cfg(target_os = "windows")]
    {
        PathBuf::from(r"\\.\pipe\aicored")
    }
}

fn default_memory_path() -> PathBuf {
    std::env::temp_dir().join("aicore").join("memory.db")
}

fn default_plugins_dir() -> PathBuf {
    std::env::temp_dir().join("aicore").join("plugins")
}

fn main() -> ExitCode {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(e) => {
            if e == cli::HELP {
                println!("{}", e);
                return ExitCode::SUCCESS;
            }
            eprintln!("error: {}", e);
            eprintln!("\n{}", cli::HELP);
            return ExitCode::from(2);
        },
    };

    let config_path = args.config_path.clone().unwrap_or_else(default_config_path);
    let mut app_config: AicoreConfig = match load_from_file(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("invalid config {}: {}", config_path.display(), e);
            return ExitCode::from(2);
        },
    };
    apply_env_overrides(&mut app_config);
    if let Some(ms) = args.interval_ms {
        app_config.daemon.telemetry.sampling_interval_ms = ms;
    }
    if let Err(e) = app_config.validate() {
        eprintln!("invalid config: {}", e);
        return ExitCode::from(2);
    }

    let effective_log = args.log_format.or(if app_config.logging_json {
        Some(aicore_runtime::LogFormat::Json)
    } else {
        Some(aicore_runtime::LogFormat::Plain)
    });

    aicore_runtime::init_tracing(effective_log);

    let config = app_config.daemon.clone();
    let state_path: PathBuf = args.state_path.clone().unwrap_or_else(default_state_path);
    let ipc_path: PathBuf = args.ipc_path.clone().unwrap_or_else(default_ipc_path);
    let memory_path: PathBuf = args
        .memory_path
        .clone()
        .or_else(|| app_config.memory_path.clone().map(PathBuf::from))
        .unwrap_or_else(default_memory_path);
    let plugins_dir: PathBuf = args
        .plugins_dir
        .clone()
        .or_else(|| app_config.plugins_dir.clone().map(PathBuf::from))
        .unwrap_or_else(default_plugins_dir);

    match aicore_memory::migrate_with_defaults() {
        Ok(report) if !report.skipped => {
            tracing::info!(
                component = "aicored",
                event = "memory_migrated",
                records = report.records_migrated,
                profile = report.profile_migrated,
                "migrated V1 memory to V2 SQLite"
            );
        },
        Ok(report) => {
            tracing::debug!(
                component = "aicored",
                event = "memory_migration_skipped",
                reason = ?report.skip_reason,
                "memory migration skipped"
            );
        },
        Err(e) => {
            tracing::warn!(
                component = "aicored",
                event = "memory_migration_failed",
                error = %e,
                "memory migration failed; continuing"
            );
        },
    }

    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("failed to build tokio runtime: {}", e);
            return ExitCode::from(1);
        },
    };

    let shared_task_queue: Arc<dyn ITaskQueue> = Arc::new(InMemoryTaskQueue::new());

    let once = args.once;

    let result = rt.block_on(async move {
        let daemon_runtime =
            runtime::build_runtime_full(Some(&memory_path), Some(shared_task_queue.clone()));

        let (tx, rx) = mpsc::channel::<()>(1);
        if !once {
            lifecycle::spawn_signal_listener(tx);
        }

        scheduler::run(scheduler::SchedulerConfig {
            runtime: daemon_runtime,
            config,
            state_path,
            ipc_path,
            plugins_dir,
            task_queue: shared_task_queue.clone(),
            once,
            shutdown_rx: rx,
        })
        .await
    });

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("daemon error: {}", e);
            ExitCode::from(1)
        },
    }
}
