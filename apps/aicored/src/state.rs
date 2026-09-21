use std::path::{Path, PathBuf};

use aicore_contracts::runtime::daemon_status::DaemonStatus;

/// 默认状态文件路径。
pub fn default_state_path() -> PathBuf {
    std::env::temp_dir().join("aicore").join("status.json")
}

/// 原子写状态文件。
///
/// 先写 `.tmp` 再 rename，避免读到半截文件。
pub fn write_status_atomic(path: &Path, status: &DaemonStatus) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp_path = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(status)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(&tmp_path, json)?;
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}
