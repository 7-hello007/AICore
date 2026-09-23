#[cfg(target_os = "windows")]
use std::process::Command;

use aicore_contracts::platform::service::{
    IServiceController, ServiceConfig, ServiceOperationResult, ServiceState,
};

/// Windows Service Controller。
///
/// 通过 `sc.exe` 实现；不引入 `windows-service` crate（MSRV 稳定）。
///
/// 在非 Windows 平台上所有操作返回 `NotSupported`。
pub struct WindowsServiceController {
    #[cfg(target_os = "windows")]
    config: std::sync::Mutex<Option<ServiceConfig>>,
}

impl WindowsServiceController {
    pub fn new() -> Self {
        #[cfg(target_os = "windows")]
        {
            return Self {
                config: std::sync::Mutex::new(None),
            };
        }

        #[cfg(not(target_os = "windows"))]
        {
            Self {}
        }
    }

    #[cfg(target_os = "windows")]
    fn stored_config(&self) -> Option<ServiceConfig> {
        self.config.lock().ok().and_then(|c| c.clone())
    }

    #[cfg(target_os = "windows")]
    fn service_name(&self) -> String {
        self.stored_config()
            .map(|c| c.name)
            .unwrap_or_else(|| "aicored".to_string())
    }

    #[cfg(target_os = "windows")]
    fn binary_path(&self) -> Option<String> {
        self.stored_config().map(|c| c.binary_path)
    }
}

impl Default for WindowsServiceController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_os = "windows")]
fn run_sc(args: &[&str]) -> std::io::Result<std::process::Output> {
    Command::new("sc.exe").args(args).output()
}

#[cfg(target_os = "windows")]
fn quote_windows_arg(arg: &str) -> String {
    if !arg.chars().any(|c| c.is_whitespace() || c == '"') {
        return arg.to_string();
    }
    format!("\"{}\"", arg.replace('"', "\\\""))
}

#[cfg(target_os = "windows")]
fn parse_sc_state(text: &str) -> ServiceState {
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("STATE") {
            continue;
        }
        let rhs = trimmed
            .split_once(':')
            .map(|(_, rhs)| rhs)
            .unwrap_or(trimmed);
        let token = rhs
            .split_whitespace()
            .nth(1)
            .or_else(|| rhs.split_whitespace().next())
            .unwrap_or("");
        return match token {
            "RUNNING" => ServiceState::Running,
            "STOPPED" => ServiceState::Stopped,
            "START_PENDING" => ServiceState::Starting,
            "STOP_PENDING" => ServiceState::Stopping,
            _ => ServiceState::Unknown,
        };
    }
    ServiceState::Unknown
}

#[cfg(not(target_os = "windows"))]
fn not_supported() -> ServiceOperationResult {
    ServiceOperationResult::failed(
        ServiceState::Unknown,
        "WindowsServiceController only available on Windows",
    )
}

impl IServiceController for WindowsServiceController {
    fn install(&self, config: &ServiceConfig) -> ServiceOperationResult {
        let _ = config;
        #[cfg(target_os = "windows")]
        {
            if let Err(e) = config.validate() {
                return ServiceOperationResult::failed(ServiceState::Failed, e.to_string());
            }
            if let Ok(mut slot) = self.config.lock() {
                *slot = Some(config.clone());
            }

            let bin_path = if config.args.is_empty() {
                format!("\"{}\"", config.binary_path)
            } else {
                let args = config
                    .args
                    .iter()
                    .map(|arg| quote_windows_arg(arg))
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("\"{}\" {}", config.binary_path, args)
            };

            let start = if config.auto_start { "auto" } else { "demand" };
            match run_sc(&[
                "create",
                &config.name,
                &format!("binPath= {}", bin_path),
                &format!("start= {}", start),
                &format!("DisplayName= {}", config.display_name),
            ]) {
                Ok(out) if out.status.success() => {
                    ServiceOperationResult::ok(ServiceState::Stopped)
                },
                Ok(out) => ServiceOperationResult::failed(
                    ServiceState::Failed,
                    String::from_utf8_lossy(&out.stderr).to_string(),
                ),
                Err(e) => ServiceOperationResult::failed(ServiceState::Failed, e.to_string()),
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            not_supported()
        }
    }

    fn uninstall(&self) -> ServiceOperationResult {
        #[cfg(target_os = "windows")]
        {
            match run_sc(&["delete", &self.service_name()]) {
                Ok(out) if out.status.success() => {
                    ServiceOperationResult::ok(ServiceState::NotInstalled)
                },
                Ok(out) => ServiceOperationResult::failed(
                    ServiceState::Failed,
                    String::from_utf8_lossy(&out.stderr).to_string(),
                ),
                Err(e) => ServiceOperationResult::failed(ServiceState::Failed, e.to_string()),
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            not_supported()
        }
    }

    fn start(&self) -> ServiceOperationResult {
        #[cfg(target_os = "windows")]
        {
            match run_sc(&["start", &self.service_name()]) {
                Ok(out) if out.status.success() => {
                    ServiceOperationResult::ok(ServiceState::Starting)
                },
                Ok(out) => ServiceOperationResult::failed(
                    ServiceState::Failed,
                    String::from_utf8_lossy(&out.stderr).to_string(),
                ),
                Err(e) => ServiceOperationResult::failed(ServiceState::Failed, e.to_string()),
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            not_supported()
        }
    }

    fn stop(&self) -> ServiceOperationResult {
        #[cfg(target_os = "windows")]
        {
            match run_sc(&["stop", &self.service_name()]) {
                Ok(out) if out.status.success() => {
                    ServiceOperationResult::ok(ServiceState::Stopping)
                },
                Ok(out) => ServiceOperationResult::failed(
                    ServiceState::Failed,
                    String::from_utf8_lossy(&out.stderr).to_string(),
                ),
                Err(e) => ServiceOperationResult::failed(ServiceState::Failed, e.to_string()),
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            not_supported()
        }
    }

    fn upgrade(&self, new_binary_path: &str) -> ServiceOperationResult {
        let _ = new_binary_path;
        #[cfg(target_os = "windows")]
        {
            // 备份当前二进制。优先使用 install() 保存的配置。
            let current = match self.binary_path() {
                Some(path) => path,
                None => {
                    return ServiceOperationResult::failed(
                        ServiceState::Failed,
                        "service binary path is not configured; install the service first",
                    )
                },
            };
            let backup = format!("{}.bak", current);
            if let Err(e) = std::fs::copy(&current, &backup) {
                return ServiceOperationResult::failed(
                    ServiceState::Failed,
                    format!("backup failed: {}", e),
                );
            }
            match std::fs::copy(new_binary_path, &current) {
                Ok(_) => ServiceOperationResult::ok(ServiceState::Stopped),
                Err(e) => ServiceOperationResult::failed(
                    ServiceState::Failed,
                    format!("copy failed: {}", e),
                ),
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            not_supported()
        }
    }

    fn rollback(&self) -> ServiceOperationResult {
        #[cfg(target_os = "windows")]
        {
            let current = match self.binary_path() {
                Some(path) => path,
                None => {
                    return ServiceOperationResult::failed(
                        ServiceState::Failed,
                        "service binary path is not configured; install the service first",
                    )
                },
            };
            let backup = format!("{}.bak", current);
            match std::fs::copy(&backup, &current) {
                Ok(_) => ServiceOperationResult::ok(ServiceState::Stopped),
                Err(e) => ServiceOperationResult::failed(
                    ServiceState::Failed,
                    format!("rollback failed: {}", e),
                ),
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            not_supported()
        }
    }

    fn query_state(&self) -> ServiceState {
        #[cfg(target_os = "windows")]
        {
            match run_sc(&["query", &self.service_name()]) {
                Ok(out) if out.status.success() => {
                    let text = String::from_utf8_lossy(&out.stdout);
                    parse_sc_state(&text)
                },
                _ => ServiceState::NotInstalled,
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            ServiceState::NotInstalled
        }
    }

    fn controller_name(&self) -> &'static str {
        "windows_service_controller"
    }
}
