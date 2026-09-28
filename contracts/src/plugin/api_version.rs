use serde::{Deserialize, Serialize};

/// Plugin API version.
///
/// Main version is not compatible: Core refuses to load plugins whose main version does not match.
/// Sub-version is compatible down: Core can load the plugin for sub-version ≤ itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiVersion {
    pub major: u32,
    pub minor: u32,
}

impl ApiVersion {
    pub const fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }

    /// The current version of Plugin API supported by Core.
    pub const CURRENT: ApiVersion = ApiVersion::new(1, 0);

    /// Compatibility: The main version is the same and plugin sub-version ≤ Core.
    pub fn is_compatible_with(&self, core: ApiVersion) -> bool {
        self.major == core.major && self.minor <= core.minor
    }

    pub fn to_string_repr(&self) -> String {
        format!("{}.{}", self.major, self.minor)
    }
}

impl std::fmt::Display for ApiVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// Parsing from "1.0".
pub fn parse_api_version(s: &str) -> Option<ApiVersion> {
    let mut parts = s.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(ApiVersion { major, minor })
}
