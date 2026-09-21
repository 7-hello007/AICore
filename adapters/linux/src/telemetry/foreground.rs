//! Linux foreground-window detection with bounded caching.
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default)]
pub struct ForegroundInfo {
    pub window_title: Option<String>,
    pub pid: Option<u32>,
}

pub struct ForegroundCache {
    last_read: Mutex<Option<Instant>>,
    last_value: Mutex<ForegroundInfo>,
    cache_interval: Duration,
    backend: ForegroundBackend,
}

#[derive(Debug, Clone, Copy)]
enum ForegroundBackend {
    Sway,
    Hyprland,
    Xdotool,
    None,
}

impl ForegroundCache {
    pub fn new(cache_interval_ms: u64) -> Self {
        Self {
            last_read: Mutex::new(None),
            last_value: Mutex::new(ForegroundInfo::default()),
            cache_interval: Duration::from_millis(cache_interval_ms.max(50)),
            backend: detect_backend(),
        }
    }

    pub fn is_available(&self) -> bool {
        !matches!(self.backend, ForegroundBackend::None)
    }

    pub fn read(&self) -> ForegroundInfo {
        if !self.is_available() {
            return ForegroundInfo::default();
        }
        if let Ok(last) = self.last_read.lock() {
            if last
                .map(|t| t.elapsed() < self.cache_interval)
                .unwrap_or(false)
            {
                return self
                    .last_value
                    .lock()
                    .map(|v| v.clone())
                    .unwrap_or_default();
            }
        }
        let info = read_with_backend(self.backend);
        if let Ok(mut last) = self.last_read.lock() {
            *last = Some(Instant::now());
        }
        if let Ok(mut value) = self.last_value.lock() {
            *value = info.clone();
        }
        info
    }
}

fn detect_backend() -> ForegroundBackend {
    if std::env::var_os("SWAYSOCK").is_some() && command_ok("swaymsg", &["-t", "get_tree"]) {
        return ForegroundBackend::Sway;
    }
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some()
        && command_ok("hyprctl", &["activewindow", "-j"])
    {
        return ForegroundBackend::Hyprland;
    }
    if command_ok("xdotool", &["--version"]) {
        return ForegroundBackend::Xdotool;
    }
    ForegroundBackend::None
}

fn command_ok(cmd: &str, args: &[&str]) -> bool {
    Command::new(cmd)
        .args(args)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn read_with_backend(backend: ForegroundBackend) -> ForegroundInfo {
    match backend {
        ForegroundBackend::Xdotool => {
            let title = run_command("xdotool", &["getactivewindow", "getwindowname"]);
            let pid = run_command("xdotool", &["getactivewindow", "getwindowpid"])
                .and_then(|s| s.parse().ok());
            ForegroundInfo {
                window_title: title,
                pid,
            }
        },
        ForegroundBackend::Sway => parse_sway_active(run_command("swaymsg", &["-t", "get_tree"])),
        ForegroundBackend::Hyprland => {
            parse_hypr_active(run_command("hyprctl", &["activewindow", "-j"]))
        },
        ForegroundBackend::None => ForegroundInfo::default(),
    }
}

fn run_command(cmd: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(cmd).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!text.is_empty()).then_some(text)
}

fn parse_sway_active(raw: Option<String>) -> ForegroundInfo {
    let Some(text) = raw else {
        return ForegroundInfo::default();
    };
    let title = find_json_string(&text, "name");
    let pid = find_json_u32(&text, "pid");
    ForegroundInfo {
        window_title: title,
        pid,
    }
}

fn parse_hypr_active(raw: Option<String>) -> ForegroundInfo {
    let Some(text) = raw else {
        return ForegroundInfo::default();
    };
    let title = find_json_string(&text, "title");
    let pid = find_json_u32(&text, "pid");
    ForegroundInfo {
        window_title: title,
        pid,
    }
}

fn find_json_string(text: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\":\"", key);
    let start = text.find(&needle)? + needle.len();
    let rest = &text[start..];
    let end = rest.find('\"')?;
    Some(rest[..end].replace("\\\"", "\""))
}

fn find_json_u32(text: &str, key: &str) -> Option<u32> {
    let needle = format!("\"{}\":", key);
    let start = text.find(&needle)? + needle.len();
    let rest = &text[start..];
    let value = rest.split(|c: char| !c.is_ascii_digit()).next()?;
    value.parse().ok()
}
