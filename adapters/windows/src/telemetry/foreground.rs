//! Foreground window identification (Windows).

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
}

impl ForegroundCache {
    pub fn new(cache_interval_ms: u64) -> Self {
        Self {
            last_read: Mutex::new(None),
            last_value: Mutex::new(ForegroundInfo::default()),
            cache_interval: Duration::from_millis(cache_interval_ms),
        }
    }

    pub fn is_available(&self) -> bool {
        cfg!(target_os = "windows")
    }

    pub fn read(&self) -> ForegroundInfo {
        // Cache Check
        if let Ok(last) = self.last_read.lock() {
            if let Some(t) = *last {
                if t.elapsed() < self.cache_interval {
                    if let Ok(v) = self.last_value.lock() {
                        return v.clone();
                    }
                }
            }
        }

        let info = read_foreground_now();

        if let Ok(mut last) = self.last_read.lock() {
            *last = Some(Instant::now());
        }
        if let Ok(mut v) = self.last_value.lock() {
            *v = info.clone();
        }

        info
    }
}

#[cfg(target_os = "windows")]
fn read_foreground_now() -> ForegroundInfo {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
    };

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return ForegroundInfo::default();
        }

        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);

        // Window Title
        let mut buf: [u16; 512] = [0; 512];
        let len = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        let title = if len > 0 {
            String::from_utf16_lossy(&buf[..len as usize])
        } else {
            String::new()
        };

        ForegroundInfo {
            window_title: if title.is_empty() { None } else { Some(title) },
            pid: if pid == 0 { None } else { Some(pid) },
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn read_foreground_now() -> ForegroundInfo {
    ForegroundInfo::default()
}
