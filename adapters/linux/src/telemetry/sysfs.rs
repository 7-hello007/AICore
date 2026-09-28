//! Sysfs read aids.
//!
//! All functions do "failed back None", not panic.

use std::fs;
use std::path::Path;

/// Reads a file, returns a string after the trim.
pub fn read_trimmed(path: impl AsRef<Path>) -> Option<String> {
    let content = fs::read_to_string(path.as_ref()).ok()?;
    Some(content.trim().to_string())
}

/// Read a file and interpret it to i64.
pub fn read_i64(path: impl AsRef<Path>) -> Option<i64> {
    let s = read_trimmed(path)?;
    s.parse().ok()
}

/// Reads a file and resolves it to f32.
pub fn read_f32(path: impl AsRef<Path>) -> Option<f32> {
    let s = read_trimmed(path)?;
    s.parse().ok()
}

/// Read a file and interpret it to u64.
pub fn read_u64(path: impl AsRef<Path>) -> Option<u64> {
    let s = read_trimmed(path)?;
    s.parse().ok()
}

/// Lists the names of all entries in the directory (non-renewal).
pub fn list_dir(path: impl AsRef<Path>) -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(entries) = fs::read_dir(path.as_ref()) {
        for e in entries.flatten() {
            if let Some(name) = e.file_name().to_str() {
                names.push(name.to_string());
            }
        }
    }
    names.sort();
    names
}
