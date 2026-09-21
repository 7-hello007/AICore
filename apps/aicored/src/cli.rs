use std::path::PathBuf;

use aicore_runtime::LogFormat;

#[derive(Debug, Clone, Default)]
pub struct CliArgs {
    pub config_path: Option<PathBuf>,
    pub interval_ms: Option<u64>,
    pub state_path: Option<PathBuf>,
    pub ipc_path: Option<PathBuf>,
    pub memory_path: Option<PathBuf>,
    pub plugins_dir: Option<PathBuf>,
    pub log_format: Option<LogFormat>,
    pub once: bool,
}

pub const HELP: &str = "\
aicored — AICore daemon

USAGE:
    aicored [OPTIONS]

OPTIONS:
    --config <PATH>         Configuration file (default: temp dir/aicore/config.json)
    --interval-ms <MS>      Tick interval in milliseconds (default 100)
    --state-path <PATH>     Status file path (default: temp dir/aicore/status.json)
    --ipc-path <PATH>       IPC socket path (default: temp dir/aicore/aicored.sock)
    --memory-path <PATH>    SQLite memory path (default: temp dir/aicore/memory.db)
    --plugins-dir <PATH>    Plugins directory (default: temp dir/aicore/plugins)
    --log-format <FMT>      plain | json (default plain)
    --once                  Run exactly one tick and exit
    -h, --help              Print help
";

pub fn parse_args<I, S>(args: I) -> Result<CliArgs, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut result = CliArgs::default();
    let mut iter = args.into_iter().map(Into::into);

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                return Err(HELP.to_string());
            },
            "--once" => {
                result.once = true;
            },
            "--config" => {
                let v = iter.next().ok_or("--config requires a value")?;
                result.config_path = Some(PathBuf::from(v));
            },
            "--interval-ms" => {
                let v = iter.next().ok_or("--interval-ms requires a value")?;
                let n: u64 = v
                    .parse()
                    .map_err(|e| format!("invalid --interval-ms: {}", e))?;
                result.interval_ms = Some(n);
            },
            "--state-path" => {
                let v = iter.next().ok_or("--state-path requires a value")?;
                result.state_path = Some(PathBuf::from(v));
            },
            "--ipc-path" => {
                let v = iter.next().ok_or("--ipc-path requires a value")?;
                result.ipc_path = Some(PathBuf::from(v));
            },
            "--memory-path" => {
                let v = iter.next().ok_or("--memory-path requires a value")?;
                result.memory_path = Some(PathBuf::from(v));
            },
            "--plugins-dir" => {
                let v = iter.next().ok_or("--plugins-dir requires a value")?;
                result.plugins_dir = Some(PathBuf::from(v));
            },
            "--log-format" => {
                let v = iter.next().ok_or("--log-format requires a value")?;
                result.log_format = Some(match v.as_str() {
                    "plain" => LogFormat::Plain,
                    "json" => LogFormat::Json,
                    other => return Err(format!("unknown log format: {}", other)),
                });
            },
            other => return Err(format!("unknown argument: {}", other)),
        }
    }

    Ok(result)
}
