use std::path::{Path, PathBuf};

use aicore_contracts::runtime::daemon_status::DaemonStatus;

/// Default status file path.
pub fn default_state_path() -> PathBuf {
    std::env::temp_dir().join("aicore").join("status.json")
}

/// Atomic writing status file.
///
/// Write `.tmp` and rename to avoid reading semi-document.
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
