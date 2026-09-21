use std::sync::Once;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

static INIT: Once = Once::new();

/// 日志输出格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Plain,
    Json,
}

/// 初始化 tracing。
///
/// 行为：
/// - 幂等：多次调用只初始化一次，不 panic。
/// - 从 `AICORE_LOG` 读过滤器，默认 `info`。
/// - 从 `AICORE_LOG_FORMAT` 读格式，`json` 时启用 JSON 输出。
///
/// 参数 `format_override` 为 `Some` 时覆盖环境变量。
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

/// 便捷：从环境变量初始化。
pub fn init_tracing_from_env() {
    init_tracing(None);
}
