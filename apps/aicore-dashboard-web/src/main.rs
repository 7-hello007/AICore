use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use aicore_dashboard::DashboardClient;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;

mod routes;
mod sse;

const HELP: &str = "\
aicore-dashboard-web — AICore local web dashboard

USAGE:
    aicore-dashboard-web [OPTIONS]

OPTIONS:
    --listen <ADDR>         Listen address (default 127.0.0.1:7878)
    --ipc-path <PATH>       Daemon IPC path (default: temp dir/aicore/aicored.sock)
    --refresh-ms <MS>       Snapshot refresh interval (default 1000, min 200)
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

struct Args {
    listen: SocketAddr,
    ipc_path: PathBuf,
    refresh_ms: u64,
}

fn parse_args() -> Result<Args, String> {
    let raw: Vec<String> = std::env::args().skip(1).collect();

    if raw.iter().any(|a| a == "-h" || a == "--help") {
        return Err(HELP.to_string());
    }

    let mut listen: SocketAddr = "127.0.0.1:7878".parse().unwrap();
    let mut ipc_path = default_ipc_path();
    let mut refresh_ms: u64 = 1000;

    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--listen" => {
                i += 1;
                if i >= raw.len() {
                    return Err("--listen requires a value".to_string());
                }
                listen = raw[i]
                    .parse()
                    .map_err(|e| format!("invalid --listen: {}", e))?;
            },
            "--ipc-path" => {
                i += 1;
                if i >= raw.len() {
                    return Err("--ipc-path requires a value".to_string());
                }
                ipc_path = PathBuf::from(&raw[i]);
            },
            "--refresh-ms" => {
                i += 1;
                if i >= raw.len() {
                    return Err("--refresh-ms requires a value".to_string());
                }
                refresh_ms = raw[i]
                    .parse()
                    .map_err(|e| format!("invalid --refresh-ms: {}", e))?;
                if refresh_ms < 200 {
                    return Err("--refresh-ms must be >= 200".to_string());
                }
            },
            other => return Err(format!("unknown argument: {}", other)),
        }
        i += 1;
    }

    Ok(Args {
        listen,
        ipc_path,
        refresh_ms,
    })
}

#[derive(Clone)]
pub struct DashboardEvent {
    pub id: u64,
    pub payload: String,
}

pub struct AppState {
    pub client: Arc<DashboardClient>,
    pub event_tx: broadcast::Sender<DashboardEvent>,
    pub event_seq: AtomicU64,
    pub refresh_ms: u64,
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

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

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

    rt.block_on(async_main(args))
}

async fn async_main(args: Args) -> ExitCode {
    let (event_tx, _) = broadcast::channel::<DashboardEvent>(64);
    let state = Arc::new(AppState {
        client: Arc::new(DashboardClient::new(args.ipc_path.clone())),
        event_tx: event_tx.clone(),
        event_seq: AtomicU64::new(1),
        refresh_ms: args.refresh_ms,
    });

    // Background tasks: periodic snapshotting and broadcasting
    {
        let state = state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(state.refresh_ms));
            loop {
                interval.tick().await;
                if state.event_tx.receiver_count() == 0 {
                    continue;
                }
                let snap = state.client.fetch().await;
                if let Ok(json) = serde_json::to_string(&snap) {
                    let id = state.event_seq.fetch_add(1, Ordering::Relaxed);
                    let _ = state.event_tx.send(DashboardEvent { id, payload: json });
                }
            }
        });
    }

    if !args.listen.ip().is_loopback()
        && std::env::var("AICORE_DASHBOARD_ALLOW_REMOTE")
            .ok()
            .as_deref()
            != Some("1")
    {
        eprintln!("refusing non-loopback dashboard bind; set AICORE_DASHBOARD_ALLOW_REMOTE=1 only when remote access is explicitly required");
        return ExitCode::from(2);
    }

    let listener = match TcpListener::bind(args.listen).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("failed to bind {}: {}", args.listen, e);
            return ExitCode::from(1);
        },
    };

    tracing::info!(
        component = "dashboard_web",
        event = "listening",
        addr = %args.listen,
        ipc_path = %args.ipc_path.display(),
        "web dashboard listening"
    );

    println!("AICore Web Dashboard: http://{}", args.listen);

    // Ctrl+C
    let shutdown = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            _ = &mut shutdown => {
                tracing::info!(component = "dashboard_web", event = "shutdown", "shutting down");
                break;
            }
            accept = listener.accept() => {
                match accept {
                    Ok((stream, addr)) => {
                        let state = state.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(stream, state).await {
                                tracing::debug!(
                                    component = "dashboard_web",
                                    event = "connection_error",
                                    peer = %addr,
                                    error = %e,
                                    "connection ended"
                                );
                            }
                        });
                    }
                    Err(e) => {
                        tracing::warn!(
                            component = "dashboard_web",
                            event = "accept_failed",
                            error = %e,
                            "accept failed"
                        );
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                }
            }
        }
    }

    ExitCode::SUCCESS
}

async fn handle_connection(mut stream: TcpStream, state: Arc<AppState>) -> std::io::Result<()> {
    const MAX_HEADER_BYTES: usize = 32 * 1024;
    let mut buf = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];
    loop {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            if buf.is_empty() {
                return Ok(());
            }
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.len() > MAX_HEADER_BYTES {
            write_simple(&mut stream, 431, "text/plain", "request headers too large").await?;
            return Ok(());
        }
        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }
    let request = String::from_utf8_lossy(&buf);
    let first_line = request.lines().next().unwrap_or("");
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("/");

    if method != "GET" {
        write_simple(&mut stream, 405, "text/plain", "method not allowed").await?;
        return Ok(());
    }

    routes::dispatch(&mut stream, path, state).await
}

pub async fn write_simple(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &str,
) -> std::io::Result<()> {
    let status_text = match status {
        200 => "OK",
        404 => "Not Found",
        405 => "Method Not Allowed",
        431 => "Request Header Fields Too Large",
        500 => "Internal Server Error",
        _ => "Unknown",
    };
    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status,
        status_text,
        content_type,
        body.len(),
        body
    );
    stream.write_all(response.as_bytes()).await?;
    stream.flush().await?;
    Ok(())
}
