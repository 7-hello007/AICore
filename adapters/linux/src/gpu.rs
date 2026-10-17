use std::path::Path;

use aicore_contracts::{
    errors::{ContractError, ContractResult},
    traits::gpu_controller::{GpuControlStatus, IGpuController},
};

const DRM_BASE: &str = "/sys/class/drm";

/// Linux GPU controller.
///
/// AMD amdgpu exposed generic performance-level controls; other manufacturers are limited in their ability to maintain them, and they are not allowed to use them.
/// Do not falsify the “controlled” state. Real vendor-specific controls are provided by hardware executor / plugin.
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
        if !matches!(
            hint,
            "compute" | "performance" | "graphics" | "auto" | "balanced" | "power_save" | "low"
        ) {
            return Err(ContractError::InvalidRequest(format!(
                "unsupported gpu hint: {hint}"
            )));
        }
        Err(ContractError::CapabilityUnavailable(
            "generic GPU setter has no authorization, independent readback or exact original-state restoration; use a reversible guarded actuator".into(),
        ))
    }

    fn get_status(&self) -> ContractResult<GpuControlStatus> {
        Ok(GpuControlStatus {
            controllable: false,
            detail: Some(format!(
                "{} GPU performance paths detected; reversible guarded control unavailable",
                Self::amd_perf_paths().len()
            )),
        })
    }

    fn capabilities(&self) -> Vec<&'static str> {
        vec!["gpu.status"]
    }

    fn platform_name(&self) -> &'static str {
        "linux"
    }
}

#[allow(dead_code)]
fn _exists(path: &str) -> bool {
    Path::new(path).exists()
}
