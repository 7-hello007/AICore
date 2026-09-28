//! AICore common schema version.
//!
//! To mark cross-module boundaries (IPC/ status file/ configuration file / Dashboard snapshot)
//! .
//!
//! Format
//!
//! - `Display`/`Serialize` output `"MAJOR.MINOR"`, for example, `"1.0"`.
//! - `FromStr` / `Deserialize` Accepted:
//!   - `"1"` → is considered `1.0`.
//!   - `"1.0"` → `1.0`
//!   - `"2.5"` → `2.5`
//! - Empty string/ illegal number `SchemaVersionParseError`.
//!
//! The difference between ApiVersion
//!
//! - `SchemaVersion` description** data structure** version.
//! - `ApiVersion` description** version API** with `is_compatible_with` compatibility rules.
//!   They are different and not combined.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Generic schema version.
///
/// `Default` = `1.0`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SchemaVersion {
    pub major: u32,
    pub minor: u32,
}

impl SchemaVersion {
    /// Construct a new schema version.
    pub const fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }

    /// AICore currently uses the schema version.
    pub const CURRENT: SchemaVersion = SchemaVersion::new(1, 0);

    /// is the current version.
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

/// `SchemaVersion` parsing error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SchemaVersionParseError {
    #[error("schema version is empty")]
    Empty,
    #[error("invalid major version")]
    InvalidMajor,
    #[error("invalid minor version")]
    InvalidMinor,
}
