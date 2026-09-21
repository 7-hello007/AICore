use aicore_contracts::{
    errors::ContractResult,
    traits::hardware_executor::{
        HardwareExecutionHint, HardwareExecutionRequest, HardwareExecutionResult,
        HardwareExecutionStatus, IHardwareExecutor,
    },
};
use std::process::Command;
use std::time::Instant;

/// Windows 硬件执行器。
///
/// 通用 GPU/NPU 控制没有跨厂商标准，因此这里采用 capability-gated 实现：
/// - NVIDIA：通过 nvidia-smi 提供功耗上限控制。
/// - 其他设备：明确返回 NotSupported，不伪造执行成功。
pub struct WindowsHardwareExecutor;
impl WindowsHardwareExecutor {
    pub fn new() -> Self {
        Self
    }
}
impl Default for WindowsHardwareExecutor {
    fn default() -> Self {
        Self::new()
    }
}

fn nvidia_apply(device_id: &str, hint: &HardwareExecutionHint) -> Option<HardwareExecutionResult> {
    let index = device_id.strip_prefix("nvidia:")?;
    let start = Instant::now();
    match hint {
        HardwareExecutionHint::MaxPowerWatts(w) => {
            let out = Command::new("nvidia-smi")
                .args(["-i", index, "-pl", &format!("{:.0}", w)])
                .output();
            let status = match out {
                Ok(o) if o.status.success() => HardwareExecutionStatus::Applied,
                Ok(o) => HardwareExecutionStatus::Failed {
                    reason: String::from_utf8_lossy(&o.stderr).trim().to_string(),
                },
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    HardwareExecutionStatus::NotSupported {
                        reason: "nvidia-smi not available".to_string(),
                    }
                },
                Err(e) => HardwareExecutionStatus::Failed {
                    reason: e.to_string(),
                },
            };
            Some(HardwareExecutionResult {
                device_id: device_id.to_string(),
                status,
                latency_ms: start.elapsed().as_millis() as u64,
            })
        },
        HardwareExecutionHint::AutoMode => Some(HardwareExecutionResult {
            device_id: device_id.to_string(),
            status: HardwareExecutionStatus::Skipped {
                reason: "NVIDIA GPU workload mode is not changed by generic auto hint".to_string(),
            },
            latency_ms: start.elapsed().as_millis() as u64,
        }),
        _ => None,
    }
}

impl IHardwareExecutor for WindowsHardwareExecutor {
    fn apply(&self, req: &HardwareExecutionRequest) -> ContractResult<HardwareExecutionResult> {
        if let Some(r) = nvidia_apply(&req.device_id, &req.hint) {
            return Ok(r);
        }
        Ok(HardwareExecutionResult {
            device_id: req.device_id.clone(),
            status: HardwareExecutionStatus::NotSupported {
                reason: "no supported vendor-specific Windows hardware control path".to_string(),
            },
            latency_ms: 0,
        })
    }
    fn release(&self, device_id: &str) -> ContractResult<HardwareExecutionResult> {
        Ok(HardwareExecutionResult {
            device_id: device_id.to_string(),
            status: HardwareExecutionStatus::Skipped {
                reason: "release is capability-specific; no safe universal restore value"
                    .to_string(),
            },
            latency_ms: 0,
        })
    }
    fn executor_name(&self) -> &'static str {
        "windows_hardware_executor"
    }
}
