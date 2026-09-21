use serde::{Deserialize, Serialize};

/// Plugin API 版本。
///
/// 主版本不兼容：Core 拒绝加载主版本不匹配的插件。
/// 次版本向下兼容：Core 可以加载次版本 ≤ 自身次版本的插件。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiVersion {
    pub major: u32,
    pub minor: u32,
}

impl ApiVersion {
    pub const fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }

    /// 当前 Core 支持的 Plugin API 版本。
    pub const CURRENT: ApiVersion = ApiVersion::new(1, 0);

    /// 是否兼容：主版本相同且插件次版本 ≤ Core 次版本。
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

/// 从 "1.0" 解析。
pub fn parse_api_version(s: &str) -> Option<ApiVersion> {
    let mut parts = s.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(ApiVersion { major, minor })
}
