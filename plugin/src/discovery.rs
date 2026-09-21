use std::path::{Path, PathBuf};

use aicore_contracts::plugin::manifest::PluginManifest;

use crate::registry::{PluginRegistry, RegistryError};

/// 单个 manifest 文件的解析结果。
#[derive(Debug, Clone)]
pub struct DiscoveredPlugin {
    pub path: PathBuf,
    pub manifest: PluginManifest,
}

/// 发现结果。
#[derive(Debug, Clone, Default)]
pub struct DiscoveryReport {
    pub discovered: Vec<DiscoveredPlugin>,
    pub rejected: Vec<RejectedPlugin>,
}

#[derive(Debug, Clone)]
pub struct RejectedPlugin {
    pub path: PathBuf,
    pub reason: String,
}

/// 扫描目录下的 `*.plugin.json`，解析并校验 manifest。
///
/// 单个插件失败不影响其他插件。
pub fn discover_from_dir(dir: impl AsRef<Path>) -> DiscoveryReport {
    let mut report = DiscoveryReport::default();
    let dir = dir.as_ref();

    if !dir.exists() {
        return report;
    }

    tracing::info!(
        component = "plugin_discovery",
        event = "scanning_dir",
        dir = %dir.display(),
        "scanning plugin directory"
    );

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(
                component = "plugin_discovery",
                event = "read_dir_failed",
                dir = %dir.display(),
                error = %e,
                "cannot read plugin directory"
            );
            return report;
        },
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !is_manifest_file(&path) {
            continue;
        }
        match load_manifest(&path) {
            Ok(manifest) => {
                tracing::info!(
                    component = "plugin_discovery",
                    event = "manifest_accepted",
                    name = %manifest.name,
                    path = %path.display(),
                    "manifest accepted"
                );
                report.discovered.push(DiscoveredPlugin { path, manifest });
            },
            Err(reason) => {
                tracing::warn!(
                    component = "plugin_discovery",
                    event = "manifest_rejected",
                    path = %path.display(),
                    reason = %reason,
                    "manifest rejected"
                );
                report.rejected.push(RejectedPlugin { path, reason });
            },
        }
    }

    report
        .discovered
        .sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
    report
}

fn is_manifest_file(path: &Path) -> bool {
    path.extension().map(|e| e == "json").unwrap_or(false)
        && path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.ends_with(".plugin.json"))
            .unwrap_or(false)
}

fn load_manifest(path: &Path) -> Result<PluginManifest, String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("read: {}", e))?;
    let manifest: PluginManifest =
        serde_json::from_str(&content).map_err(|e| format!("parse: {}", e))?;
    manifest
        .validate()
        .map_err(|e| format!("validate: {}", e))?;
    Ok(manifest)
}

/// 把发现结果注册到 registry。
///
/// 返回成功注册的数量。
pub fn register_discovered(
    registry: &PluginRegistry,
    report: &DiscoveryReport,
) -> Result<usize, RegistryError> {
    let mut registered = 0;
    for d in &report.discovered {
        match registry.register(d.manifest.clone()) {
            Ok(()) => registered += 1,
            Err(e) => {
                tracing::warn!(
                    component = "plugin_discovery",
                    event = "register_failed",
                    name = %d.manifest.name,
                    error = %e,
                    "failed to register plugin"
                );
            },
        }
    }
    Ok(registered)
}
