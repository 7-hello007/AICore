use std::sync::Once;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

static INIT: Once = Once::new();

/// Log output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Plain,
    Json,
}

/// Initializing the traffic.
///
/// Conduct:
/// - Zen et al.: multiple calls are only initialized once, not Panic.
/// - Read filter from `AICORE_LOG`, default `info`.
/// - From `AICORE_LOG_FORMAT` Read format,`json` Enable JSON Output.
///
/// The parameter `format_override` is `Some` to overwrite the environment variable.
pub fn init_tracing(format_override: Option<LogFormat>) {
    INIT.call_once(|| {
        let filter =
            EnvFilter::try_from_env("AICORE_LOG").unwrap_or_else(|_| EnvFilter::new("info"));

        let format = format_override.unwrap_or_else(|| {
            if std::env::var("AICORE_LOG_FORMAT").as_deref() == Ok("json") {
                LogFormat::Json
            } else {
                LogFormat::Plain
            }
        });

        match format {
            LogFormat::Json => {
                tracing_subscriber::registry()
                    .with(filter)
                    .with(
                        fmt::layer()
                            .json()
                            .with_target(true)
                            .with_current_span(true),
                    )
                    .init();
            },
            LogFormat::Plain => {
                tracing_subscriber::registry()
                    .with(filter)
                    .with(fmt::layer().with_target(true))
                    .init();
            },
        }
    });
}

/// Easy: Initialize from environmental variables.
pub fn init_tracing_from_env() {
    init_tracing(None);
}
