//! AICore 通用 schema 版本。
//!
//! 用于标记跨模块边界（IPC / 状态文件 / 配置文件 / Dashboard 快照）
//! 的数据结构版本。
//!
//! ## 格式
//!
//! - `Display` / `Serialize` 输出 `"MAJOR.MINOR"`，例如 `"1.0"`。
//! - `FromStr` / `Deserialize` 接受：
//!   - `"1"` → 视为 `1.0`（向后兼容老的无点格式）
//!   - `"1.0"` → `1.0`
//!   - `"2.5"` → `2.5`
//! - 空字符串 / 非法数字 → `SchemaVersionParseError`。
//!
//! ## 与 ApiVersion 的区别
//!
//! - `SchemaVersion` 描述**数据结构**版本。
//! - `ApiVersion` 描述**插件 API** 版本，并有 `is_compatible_with` 兼容性规则。
//!   两者用途不同，不合并。

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// 通用 schema 版本。
///
/// `Default` = `1.0`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SchemaVersion {
    pub major: u32,
    pub minor: u32,
}

impl SchemaVersion {
    /// 构造一个新的 schema 版本。
    pub const fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }

    /// AICore 当前使用的 schema 版本。
    pub const CURRENT: SchemaVersion = SchemaVersion::new(1, 0);

    /// 是否为当前版本。
    pub const fn is_current(&self) -> bool {
        self.major == Self::CURRENT.major && self.minor == Self::CURRENT.minor
    }
}

impl Default for SchemaVersion {
    fn default() -> Self {
        Self::CURRENT
    }
}

impl fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

impl FromStr for SchemaVersion {
    type Err = SchemaVersionParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s.is_empty() {
            return Err(SchemaVersionParseError::Empty);
        }

        let mut parts = s.splitn(2, '.');
        let major_str = parts.next().unwrap();
        let major: u32 = major_str
            .parse()
            .map_err(|_| SchemaVersionParseError::InvalidMajor)?;

        let minor: u32 = match parts.next() {
            Some(minor_str) => minor_str
                .parse()
                .map_err(|_| SchemaVersionParseError::InvalidMinor)?,
            None => 0,
        };

        Ok(Self { major, minor })
    }
}

impl Serialize for SchemaVersion {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for SchemaVersion {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        SchemaVersion::from_str(&s).map_err(serde::de::Error::custom)
    }
}

/// `SchemaVersion` 解析错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SchemaVersionParseError {
    #[error("schema version is empty")]
    Empty,
    #[error("invalid major version")]
    InvalidMajor,
    #[error("invalid minor version")]
    InvalidMinor,
}
