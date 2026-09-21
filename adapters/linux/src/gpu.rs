use std::path::Path;

use aicore_contracts::{
    errors::{ContractError, ContractResult},
    traits::gpu_controller::{GpuControlStatus, IGpuController},
};

const DRM_BASE: &str = "/sys/class/drm";

/// Linux GPU 控制器。
///
/// 对 AMD amdgpu 暴露通用的 performance-level 控制；其他厂商保持能力受限，
/// 不伪造“可控”状态。真正的 vendor-specific 控制由硬件 executor / plugin 提供。
pub struct LinuxGpuController;

impl LinuxGpuController {
    pub fn new() -> Self {
        Self
    }

    fn amd_perf_paths() -> Vec<String> {
        let mut paths = Vec::new();
        if let Ok(entries) = std::fs::read_dir(DRM_BASE) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with("card") || name.contains('-') {
                    continue;
                }
                let path = entry
                    .path()
                    .join("device/power_dpm_force_performance_level");
                if path.exists() {
                    paths.push(path.to_string_lossy().to_string());
                }
            }
        }
        paths
    }
}
impl Default for LinuxGpuController {
    fn default() -> Self {
        Self::new()
    }
}

impl IGpuController for LinuxGpuController {
    fn set_workload_hint(&self, hint: &str) -> ContractResult<()> {
        let target = match hint {
            "compute" | "performance" => "high",
            "graphics" | "auto" | "balanced" => "auto",
            "power_save" | "low" => "low",
            _ => {
                return Err(ContractError::InvalidRequest(format!(
                    "unsupported gpu hint: {}",
                    hint
                )))
            },
        };
        let paths = Self::amd_perf_paths();
        if paths.is_empty() {
            return Err(ContractError::CapabilityUnavailable(
                "AMD amdgpu performance control unavailable".to_string(),
            ));
        }
        for path in paths {
            std::fs::write(&path, target).map_err(|e| match e.kind() {
                std::io::ErrorKind::PermissionDenied => {
                    ContractError::PermissionDenied(format!("write {}: {}", path, e))
                },
                _ => ContractError::AdapterUnavailable(format!("write {}: {}", path, e)),
            })?;
        }
        Ok(())
    }

    fn get_status(&self) -> ContractResult<GpuControlStatus> {
        let paths = Self::amd_perf_paths();
        Ok(GpuControlStatus {
            controllable: !paths.is_empty(),
            detail: Some(if paths.is_empty() {
                "no generic GPU control path detected".to_string()
            } else {
                format!("AMD amdgpu performance control paths: {}", paths.len())
            }),
        })
    }

    fn capabilities(&self) -> Vec<&'static str> {
        let mut caps = vec!["gpu.status"];
        if !Self::amd_perf_paths().is_empty() {
            caps.push("gpu.workload_hint.write");
        }
        caps
    }

    fn platform_name(&self) -> &'static str {
        "linux"
    }
}

#[allow(dead_code)]
fn _exists(path: &str) -> bool {
    Path::new(path).exists()
}
