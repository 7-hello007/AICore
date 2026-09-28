use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use aicore_dashboard::{render, DashboardClient};

const HELP: &str = "\
aicore-dashboard — AICore terminal dashboard

USAGE:
    aicore-dashboard [OPTIONS]

OPTIONS:
    --ipc-path <PATH>       IPC socket path (default: temp dir/aicore/aicored.sock)
    --interval-ms <MS>      Refresh interval (default 1000, min 200)
    --limit <N>             Recent ticks to show (default 8, max 200)
    --once                  Render once and exit (for CI / snapshot)
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

fn terminal_width() -> usize {
    if let Ok(cols) = std::env::var("COLUMNS") {
        if let Ok(n) = cols.parse::<usize>() {
            return n.max(60);
        }
    }
    100
}

struct Args {
    ipc_path: PathBuf,
    interval_ms: u64,
    limit: usize,
    once: bool,
}

fn parse_args() -> Result<Args, String> {
    let raw: Vec<String> = std::env::args().skip(1).collect();

    if raw.iter().any(|a| a == "-h" || a == "--help") {
        return Err(HELP.to_string());
    }

    let mut ipc_path = default_ipc_path();
    let mut interval_ms: u64 = 1000;
    let mut limit: usize = 8;
    let mut once = false;

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
            "--once" => once = true,
            other => return Err(format!("unknown argument: {}", other)),
        }
        i += 1;
    }

    Ok(Args {
        ipc_path,
        interval_ms,
        limit,
        once,
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

    rt.block_on(async_main(args))
}

async fn async_main(args: Args) -> ExitCode {
    let client = DashboardClient::new(args.ipc_path.clone()).with_recent_ticks_limit(args.limit);
    let width = terminal_width();

    if args.once {
        let snap = client.fetch().await;
        print!("{}", render(&snap, width));
        return ExitCode::SUCCESS;
    }

    // First rendering immediately
    clear_screen();
    let snap = client.fetch().await;
    print!("{}", render(&snap, width));
    let _ = std::io::stdout().flush();

    let interval = Duration::from_millis(args.interval_ms);

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                clear_screen();
                println!("(aicore-dashboard exiting)");
                break;
            }
            _ = tokio::time::sleep(interval) => {
                clear_screen();
                let snap = client.fetch().await;
                print!("{}", render(&snap, width));
                let _ = std::io::stdout().flush();
            }
        }
    }

    ExitCode::SUCCESS
}

fn clear_screen() {
    print!("\x1b[2J\x1b[H");
}
