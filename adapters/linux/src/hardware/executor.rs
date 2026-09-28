use std::path::Path;
use std::time::Instant;

use aicore_contracts::{
    errors::ContractResult,
    traits::hardware_executor::{
        HardwareExecutionHint, HardwareExecutionRequest, HardwareExecutionResult,
        HardwareExecutionStatus, IHardwareExecutor,
    },
};

use crate::telemetry::sysfs;

const DRM_BASE: &str = "/sys/class/drm";

fn drm_card(device_id: &str) -> Option<&str> {
    let card = device_id.strip_prefix("drm:card")?;
    if card.is_empty() || !card.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    device_id.strip_prefix("drm:")
}

fn workload_target(hint: &HardwareExecutionHint) -> Option<&'static str> {
    match hint {
        HardwareExecutionHint::WorkloadPreference(value) => match value.as_str() {
            "compute" | "performance" => Some("high"),
            "graphics" | "auto" => Some("auto"),
            "low" | "power_save" => Some("low"),
            _ => None,
        },
        HardwareExecutionHint::GraphicsMode | HardwareExecutionHint::AutoMode => Some("auto"),
        HardwareExecutionHint::ComputeMode => Some("high"),
        HardwareExecutionHint::MaxPowerWatts(_) => None,
    }
}

/// Linux hardware executor (best effort).
///
/// Support path:
/// - AMD amdgpu: `power_dpm_force_performance_level` for `low`/`high`/`auto`
///
/// Unsupported path: returns `NotSupported` without affecting Core.
pub struct LinuxHardwareExecutor;

impl LinuxHardwareExecutor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxHardwareExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl IHardwareExecutor for LinuxHardwareExecutor {
    fn apply(&self, req: &HardwareExecutionRequest) -> ContractResult<HardwareExecutionResult> {
        let start = Instant::now();

        let card = match drm_card(&req.device_id) {
            Some(c) => c,
            None => {
                return Ok(HardwareExecutionResult {
                    device_id: req.device_id.clone(),
                    status: HardwareExecutionStatus::NotSupported {
                        reason: format!("unsupported device id: {}", req.device_id),
                    },
                    latency_ms: start.elapsed().as_millis() as u64,
                });
            },
        };

        let device_path = format!("{}/{}/device", DRM_BASE, card);
        if !Path::new(&device_path).exists() {
            return Ok(HardwareExecutionResult {
                device_id: req.device_id.clone(),
                status: HardwareExecutionStatus::NotSupported {
                    reason: format!("gpu device not found: {}", req.device_id),
                },
                latency_ms: start.elapsed().as_millis() as u64,
            });
        }

        // Check amdgpu performance level path
        let perf_path = format!("{}/power_dpm_force_performance_level", device_path);
        if !Path::new(&perf_path).exists() {
            return Ok(HardwareExecutionResult {
                device_id: req.device_id.clone(),
                status: HardwareExecutionStatus::NotSupported {
                    reason: "no power_dpm_force_performance_level (non-AMD or unsupported driver)"
                        .to_string(),
                },
                latency_ms: start.elapsed().as_millis() as u64,
            });
        }

        let target = match workload_target(&req.hint) {
            Some(target) => target,
            None => {
                return Ok(HardwareExecutionResult {
                    device_id: req.device_id.clone(),
                    status: HardwareExecutionStatus::NotSupported {
                        reason: "unsupported GPU workload hint or power cap".to_string(),
                    },
                    latency_ms: start.elapsed().as_millis() as u64,
                });
            },
        };

        // Read current value, AlreadyInState if consistent
        let current = sysfs::read_trimmed(&perf_path);
        if current.as_deref() == Some(target) {
            return Ok(HardwareExecutionResult {
                device_id: req.device_id.clone(),
                status: HardwareExecutionStatus::AlreadyInState,
                latency_ms: start.elapsed().as_millis() as u64,
            });
        }

        // Try writing
        match std::fs::write(&perf_path, target) {
            Ok(()) => Ok(HardwareExecutionResult {
                device_id: req.device_id.clone(),
                status: HardwareExecutionStatus::Applied,
                latency_ms: start.elapsed().as_millis() as u64,
            }),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                Ok(HardwareExecutionResult {
                    device_id: req.device_id.clone(),
                    status: HardwareExecutionStatus::Failed {
                        reason: format!("permission denied: {}", e),
                    },
                    latency_ms: start.elapsed().as_millis() as u64,
                })
            },
            Err(e) => Ok(HardwareExecutionResult {
                device_id: req.device_id.clone(),
                status: HardwareExecutionStatus::Failed {
                    reason: format!("write failed: {}", e),
                },
                latency_ms: start.elapsed().as_millis() as u64,
            }),
        }
    }

    fn release(&self, device_id: &str) -> ContractResult<HardwareExecutionResult> {
        let start = Instant::now();
        // Restore auto
        self.apply(&HardwareExecutionRequest {
            device_id: device_id.to_string(),
            hint: HardwareExecutionHint::AutoMode,
        })
        .map(|mut r| {
            r.latency_ms = start.elapsed().as_millis() as u64;
            r
        })
    }

    fn executor_name(&self) -> &'static str {
        "linux_hardware_executor"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_id_cannot_escape_drm_card_directory() {
        assert_eq!(drm_card("drm:card0"), Some("card0"));
        for invalid in ["drm:card", "drm:card0/device", "drm:../card0", "drm:card-1"] {
            assert_eq!(drm_card(invalid), None);
        }
    }

    #[test]
    fn unknown_workload_preference_has_no_write_target() {
        assert_eq!(
            workload_target(&HardwareExecutionHint::WorkloadPreference("unknown".into())),
            None
        );
        assert_eq!(
            workload_target(&HardwareExecutionHint::MaxPowerWatts(10.0)),
            None
        );
    }
}
