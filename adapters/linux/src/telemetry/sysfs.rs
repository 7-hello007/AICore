//! sysfs 读取辅助。
//!
//! 所有函数都做"失败返回 None"，不 panic。

use std::fs;
use std::path::Path;

/// 读一个文件，返回 trim 后的字符串。
pub fn read_trimmed(path: impl AsRef<Path>) -> Option<String> {
    let content = fs::read_to_string(path.as_ref()).ok()?;
    Some(content.trim().to_string())
}

/// 读一个文件并解析为 i64。
pub fn read_i64(path: impl AsRef<Path>) -> Option<i64> {
    let s = read_trimmed(path)?;
    s.parse().ok()
}

/// 读一个文件并解析为 f32。
pub fn read_f32(path: impl AsRef<Path>) -> Option<f32> {
    let s = read_trimmed(path)?;
    s.parse().ok()
}

/// 读一个文件并解析为 u64。
pub fn read_u64(path: impl AsRef<Path>) -> Option<u64> {
    let s = read_trimmed(path)?;
    s.parse().ok()
}

/// 列出目录下所有条目名（不递归）。
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
