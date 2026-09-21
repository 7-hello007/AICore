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

/// Linux 硬件执行器（尽力而为）。
///
/// 支持路径：
/// - AMD amdgpu：`power_dpm_force_performance_level` 写入 `low` / `high` / `auto`
///
/// 不支持路径：返回 `NotSupported`，不影响 Core。
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

        let card = match req.device_id.strip_prefix("drm:") {
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

        // 检查 amdgpu performance level 路径
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

        let target = match &req.hint {
            HardwareExecutionHint::WorkloadPreference(s) => match s.as_str() {
                "compute" | "performance" => "high",
                "graphics" | "auto" => "auto",
                "low" | "power_save" => "low",
                _ => "auto",
            },
            HardwareExecutionHint::MaxPowerWatts(_) => {
                return Ok(HardwareExecutionResult {
                    device_id: req.device_id.clone(),
                    status: HardwareExecutionStatus::NotSupported {
                        reason: "power cap not supported via sysfs here".to_string(),
                    },
                    latency_ms: start.elapsed().as_millis() as u64,
                });
            },
            HardwareExecutionHint::GraphicsMode => "auto",
            HardwareExecutionHint::ComputeMode => "high",
            HardwareExecutionHint::AutoMode => "auto",
        };

        // 读当前值，若已一致则 AlreadyInState
        let current = sysfs::read_trimmed(&perf_path);
        if current.as_deref() == Some(target) {
            return Ok(HardwareExecutionResult {
                device_id: req.device_id.clone(),
                status: HardwareExecutionStatus::AlreadyInState,
                latency_ms: start.elapsed().as_millis() as u64,
            });
        }

        // 尝试写入
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
        // 恢复 auto
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
