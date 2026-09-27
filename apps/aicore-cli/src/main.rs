use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use aicore_contracts::ipc::protocol::{Request, ResponseData};
use aicore_contracts::{ai::intent::IntentRequest, application::id::ApplicationId};
use aicore_ipc::{unwrap_response, IpcClient};

const HELP: &str = "\
aicore — AICore CLI

USAGE:
    aicore <COMMAND> [OPTIONS]

COMMANDS:
    status           Show daemon status
    capabilities     List adapter capabilities
    policy           Show latest resource policy
    context          Show latest context snapshot
    ticks            List recent ticks (default 10)
    plugins          List registered plugins
    intent           Ask the local intent provider for a semantic proposal
    watch            Live-refresh terminal dashboard

OPTIONS:
    --ipc-path <PATH>       IPC socket path (default: temp dir/aicore/aicored.sock)
    --state-path <PATH>     State file path (fallback)
    --limit <N>             ticks: number of entries (default 10, max 200)
    --interval-ms <MS>      watch: refresh interval (default 1000)
    --text <TEXT>           intent: task description (max 4096 bytes)
    --application-id <ID>   intent: application identity (default aicore.cli)
    -h, --help              Print help
";

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

fn default_state_path() -> PathBuf {
    std::env::temp_dir().join("aicore").join("status.json")
}

struct ParsedArgs {
    ipc_path: PathBuf,
    state_path: PathBuf,
    limit: usize,
    interval_ms: u64,
    command: String,
    text: Option<String>,
    application_id: String,
}

fn parse_args() -> Result<ParsedArgs, String> {
    let raw: Vec<String> = std::env::args().skip(1).collect();

    if raw.is_empty() {
        return Err(HELP.to_string());
    }

    if raw.iter().any(|a| a == "-h" || a == "--help") {
        return Err(HELP.to_string());
    }

    let mut ipc_path = default_ipc_path();
    let mut state_path = default_state_path();
    let mut limit: usize = 10;
    let mut interval_ms: u64 = 1000;
    let mut command: Option<String> = None;
    let mut text: Option<String> = None;
    let mut application_id = "aicore.cli".to_string();

    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--ipc-path" => {
                i += 1;
                if i >= raw.len() {
                    return Err("--ipc-path requires a value".to_string());
                }
                ipc_path = PathBuf::from(&raw[i]);
            },
            "--state-path" => {
                i += 1;
                if i >= raw.len() {
                    return Err("--state-path requires a value".to_string());
                }
                state_path = PathBuf::from(&raw[i]);
            },
            "--limit" => {
                i += 1;
                if i >= raw.len() {
                    return Err("--limit requires a value".to_string());
                }
                limit = raw[i]
                    .parse()
                    .map_err(|e| format!("invalid --limit: {}", e))?;
                if limit == 0 || limit > 200 {
                    return Err("--limit must be between 1 and 200".to_string());
                }
            },
            "--interval-ms" => {
                i += 1;
                if i >= raw.len() {
                    return Err("--interval-ms requires a value".to_string());
                }
                interval_ms = raw[i]
                    .parse()
                    .map_err(|e| format!("invalid --interval-ms: {}", e))?;
                if interval_ms < 200 {
                    return Err("--interval-ms must be >= 200".to_string());
                }
            },
            "--text" => {
                i += 1;
                if i >= raw.len() {
                    return Err("--text requires a value".into());
                }
                text = Some(raw[i].clone());
            },
            "--application-id" => {
                i += 1;
                if i >= raw.len() {
                    return Err("--application-id requires a value".into());
                }
                application_id = raw[i].clone();
            },
            other if other.starts_with("--") => {
                return Err(format!("unknown option: {}", other));
            },
            other => {
                if command.is_none() {
                    command = Some(other.to_string());
                } else {
                    return Err(format!("unexpected argument: {}", other));
                }
            },
        }
        i += 1;
    }

    let command = command.ok_or_else(|| "no command given".to_string())?;

    Ok(ParsedArgs {
        ipc_path,
        state_path,
        limit,
        interval_ms,
        command,
        text,
        application_id,
    })
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            if e == HELP {
                println!("{}", e);
                return ExitCode::SUCCESS;
            }
            eprintln!("error: {}", e);
            eprintln!("\n{}", HELP);
            return ExitCode::from(2);
        },
    };

    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("failed to build tokio runtime: {}", e);
            return ExitCode::from(1);
        },
    };

    match args.command.as_str() {
        "status" => rt.block_on(run_once(&args, Request::GetStatus)),
        "capabilities" => rt.block_on(run_once(&args, Request::GetCapabilities)),
        "policy" => rt.block_on(run_once(&args, Request::GetPolicy)),
        "context" => rt.block_on(run_once(&args, Request::GetContext)),
        "ticks" => rt.block_on(run_once(
            &args,
            Request::GetRecentTicks { limit: args.limit },
        )),
        "plugins" => rt.block_on(run_once(&args, Request::ListPlugins)),
        "intent" => rt.block_on(run_intent(&args)),
        "watch" => rt.block_on(run_watch(&args)),
        other => {
            eprintln!("error: unknown command: {}", other);
            eprintln!("{}", HELP);
            ExitCode::from(2)
        },
    }
}

async fn run_intent(args: &ParsedArgs) -> ExitCode {
    let Some(text) = args.text.as_ref() else {
        eprintln!("intent requires --text");
        return ExitCode::from(2);
    };
    let application_id = ApplicationId::new(&args.application_id);
    if !application_id.is_valid() || text.trim().is_empty() || text.len() > 4096 {
        eprintln!("invalid application ID or intent text");
        return ExitCode::from(2);
    }
    let mut client = match IpcClient::connect(args.ipc_path.clone()).await {
        Ok(client) => client,
        Err(error) => {
            eprintln!("IPC connect failed: {error}");
            return ExitCode::from(1);
        },
    };
    let send = |phase: &str| format!("cli-intent-{}-{phase}", std::process::id());
    let registration = Request::Register {
        application_id: application_id.clone(),
        name: "AICore CLI".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        capabilities: vec!["intent.propose".into()],
    };
    let registered = client
        .call(&send("register"), registration, Duration::from_secs(5))
        .await
        .map_err(|error| error.to_string())
        .and_then(|response| unwrap_response(response).map_err(|error| error.message));
    if let Err(error) = registered {
        eprintln!("intent registration failed: {error}");
        return ExitCode::from(1);
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0);
    let result = client
        .call(
            &send("propose"),
            Request::ProposeIntent {
                request: IntentRequest {
                    application_id: application_id.clone(),
                    text: text.clone(),
                    submitted_at_unix_ms: now,
                },
            },
            Duration::from_secs(35),
        )
        .await
        .map_err(|error| error.to_string())
        .and_then(|response| unwrap_response(response).map_err(|error| error.message));
    let _ = client
        .call(
            &send("unregister"),
            Request::Unregister { application_id },
            Duration::from_secs(5),
        )
        .await;
    match result {
        Ok(data @ ResponseData::IntentProposed { .. }) => {
            print_data(data);
            ExitCode::SUCCESS
        },
        Ok(_) => {
            eprintln!("unexpected intent response");
            ExitCode::from(1)
        },
        Err(error) => {
            eprintln!("intent request failed: {error}");
            ExitCode::from(1)
        },
    }
}

async fn run_once(args: &ParsedArgs, request: Request) -> ExitCode {
    match call_ipc(&args.ipc_path, request).await {
        Ok(data) => {
            print_data(data);
            ExitCode::SUCCESS
        },
        Err(CliError::Ipc(_)) => {
            eprintln!("(ipc unavailable, falling back to state file)");
            fallback_state_file(&args.state_path)
        },
        Err(CliError::Daemon(e)) => {
            eprintln!("daemon error: {} ({})", e.message, e.code);
            ExitCode::from(1)
        },
        Err(CliError::Transport(msg)) => {
            eprintln!("ipc call failed: {}", msg);
            ExitCode::from(1)
        },
    }
}

async fn run_watch(args: &ParsedArgs) -> ExitCode {
    let refresh = Duration::from_millis(args.interval_ms);
    loop {
        // 清屏
        print!("\x1b[2J\x1b[H");

        match call_ipc(&args.ipc_path, Request::GetStatus).await {
            Ok(ResponseData::Status { status }) => {
                render_dashboard(&status, None);
            },
            _ => {
                eprintln!("(ipc unavailable; retrying)");
                let _ = call_ipc(&args.ipc_path, Request::GetStatus).await;
            },
        }

        // 拉 ticks
        if let Ok(ResponseData::RecentTicks { ticks }) =
            call_ipc(&args.ipc_path, Request::GetRecentTicks { limit: 5 }).await
        {
            render_recent_ticks(&ticks);
        }

        println!("\n(press Ctrl+C to exit)");
        tokio::time::sleep(refresh).await;
    }
}

enum CliError {
    Ipc(String),
    Daemon(aicore_contracts::ipc::protocol::IpcError),
    Transport(String),
}

async fn call_ipc(ipc_path: &Path, request: Request) -> Result<ResponseData, CliError> {
    let mut client = IpcClient::connect(ipc_path.to_path_buf())
        .await
        .map_err(|e| CliError::Ipc(e.to_string()))?;

    let request_id = format!(
        "cli-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let resp = client
        .call(&request_id, request, Duration::from_secs(5))
        .await
        .map_err(|e| CliError::Transport(e.to_string()))?;

    unwrap_response(resp).map_err(CliError::Daemon)
}

fn fallback_state_file(path: &PathBuf) -> ExitCode {
    match std::fs::read_to_string(path) {
        Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(v) => {
                println!("{}", serde_json::to_string_pretty(&v).unwrap_or(content));
                ExitCode::SUCCESS
            },
            Err(e) => {
                eprintln!("invalid state file: {}", e);
                ExitCode::from(1)
            },
        },
        Err(e) => {
            eprintln!("cannot read state file {}: {}", path.display(), e);
            ExitCode::from(1)
        },
    }
}

fn print_data(data: ResponseData) {
    match data {
        ResponseData::HandshakeAck { .. } | ResponseData::Ack => {
            eprintln!("unexpected IPC control response");
        },
        ResponseData::Status { status } => render_dashboard(&status, None),
        ResponseData::Capabilities { component, entries } => {
            println!("component: {}", component);
            for e in entries {
                println!("  - {:?} {}", e.status, e.name);
                if let Some(d) = e.detail {
                    println!("      {}", d);
                }
            }
        },
        ResponseData::Registered { application_id } => {
            println!("registered: {}", application_id);
        },
        ResponseData::Unregistered { removed } => {
            println!("unregistered: removed={}", removed);
        },
        ResponseData::TaskSubmitted {
            accepted,
            task_id,
            message,
        } => {
            println!(
                "task submitted: accepted={} task_id={:?} message={}",
                accepted, task_id, message
            );
        },
        ResponseData::CurrentTaskDeclared => println!("current task declared"),
        ResponseData::CurrentTaskCleared { cleared } => {
            println!("current task cleared: {}", cleared);
        },
        ResponseData::IntentProposed { proposal } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&proposal).unwrap_or_default()
            );
        },
        ResponseData::PreparationScheduled {
            preparation_id,
            workload,
            ready_at_unix_ms,
        } => println!(
            "preparation scheduled: {} workload={:?} ready_at_unix_ms={}",
            preparation_id, workload, ready_at_unix_ms
        ),
        ResponseData::NativeTaskSubmitted { handle_id } => {
            println!("native task submitted: {}", handle_id);
        },
        ResponseData::NativeTaskStatus { state } => println!("native task status: {:?}", state),
        ResponseData::NativeTaskCancelled => println!("native task cancelled"),
        ResponseData::Context { context } => match context {
            Some(c) => println!("{}", serde_json::to_string_pretty(&c).unwrap()),
            None => println!("(no context captured yet)"),
        },
        ResponseData::Policy { policy } => match policy {
            Some(p) => println!("{}", serde_json::to_string_pretty(&p).unwrap()),
            None => println!("(no policy captured yet)"),
        },
        ResponseData::RecentTicks { ticks } => render_recent_ticks(&ticks),
        ResponseData::Plugins { entries } => render_plugins(&entries),
        ResponseData::DashboardSnapshot { snapshot } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&*snapshot).unwrap_or_default()
            );
        },
    }
}

fn render_dashboard(
    status: &aicore_contracts::runtime::daemon_status::DaemonStatus,
    ticks: Option<&[aicore_contracts::runtime::daemon_status::TickSummary]>,
) {
    println!("=== AICore daemon ===");
    println!("pid:              {}", status.pid);
    println!("schema version:   {}", status.version);
    println!("tick count:       {}", status.tick_count);
    println!(
        "successful:       {}   failed: {}",
        status.successful_ticks, status.failed_ticks
    );
    println!("current mode:     {:?}", status.current_mode);
    println!("last tick at:     {}", status.last_tick_at_unix_ms);

    if let Some(t) = &status.last_tick {
        println!();
        println!("=== last tick ===");
        println!("tick_id:          {}", t.tick_id);
        println!("trace_id:         {}", t.trace_id);
        println!("duration_ms:      {}", t.duration_ms);
        println!("success:          {}", t.success);
        println!("workload:         {}", t.workload_type);
        if let Some(p) = &t.policy_id {
            println!("policy_id:        {}", p);
        }
        if let Some(src) = &t.policy_source {
            println!("policy_source:    {:?}", src);
        }
        if let Some(es) = &t.execution_status {
            println!("execution:        {:?}", es);
        }
        if let Some(pw) = &t.predicted_workload {
            println!("predicted:        {:?}", pw);
        }
        if let Some(c) = t.prediction_confidence {
            println!("pred_conf:        {:.3}", c);
        }
        println!("message:          {}", t.message);
    }

    if let Some(ticks) = ticks {
        if !ticks.is_empty() {
            println!();
            render_recent_ticks(ticks);
        }
    }
}

fn render_recent_ticks(ticks: &[aicore_contracts::runtime::daemon_status::TickSummary]) {
    if ticks.is_empty() {
        println!();
        println!("(no ticks recorded yet)");
        return;
    }
    println!();
    println!("=== recent ticks ({} of {}) ===", ticks.len(), ticks.len());
    println!(
        "{:>6}  {:>8}  {:>7}  {:<16}  {:<14}  {:<18}  predicted",
        "tick", "duration", "success", "workload", "policy_source", "exec_status"
    );
    for t in ticks {
        let src = match &t.policy_source {
            Some(s) => format!("{:?}", s),
            None => "-".to_string(),
        };
        let exec = match &t.execution_status {
            Some(e) => format!("{:?}", e),
            None => "-".to_string(),
        };
        let pred = match (&t.predicted_workload, t.prediction_confidence) {
            (Some(w), Some(c)) => format!("{:?} ({:.2})", w, c),
            (Some(w), None) => format!("{:?}", w),
            _ => "-".to_string(),
        };
        println!(
            "{:>6}  {:>6}ms  {:>7}  {:<16}  {:<14}  {:<18}  {}",
            t.tick_id, t.duration_ms, t.success, t.workload_type, src, exec, pred
        );
    }
}

fn render_plugins(entries: &[aicore_contracts::ipc::protocol::PluginEntryDto]) {
    if entries.is_empty() {
        println!("(no plugins registered)");
        return;
    }
    println!("=== plugins ({}) ===", entries.len());
    for e in entries {
        println!();
        println!("  name:         {}", e.name);
        println!("  version:      {}", e.version);
        println!("  api_version:  {}", e.api_version);
        println!("  kind:         {}", e.kind);
        println!("  trust:        {}", e.trust_level);
        println!("  state:        {}", e.state);
        println!("  interfaces:   {}", e.interfaces.join(", "));
        if !e.permissions.is_empty() {
            println!("  permissions:  {}", e.permissions.join(", "));
        }
        if let Some(r) = &e.last_reason {
            println!("  reason:       {}", r);
        }
        if e.failure_count > 0 {
            println!("  failures:     {}", e.failure_count);
        }
    }
}
