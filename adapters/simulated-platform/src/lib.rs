//! Process-local platform adapter seeded only from a read-only captured fixture.
use aicore_contracts::{
    capability::entry::CapabilityEntry,
    errors::{ContractError, ContractResult},
    hardware::{
        capability::{HardwareCapability, HardwareCapabilitySet, HardwareDeviceKind},
        matrix::DeviceCapabilityMatrix,
        telemetry::{GpuTelemetrySample, HardwareDeviceInfo, NpuTelemetrySample},
    },
    policy::{priority::PolicyPriority, resource_policy::PowerMode},
    telemetry::snapshot::ComputerSnapshot,
    traits::{
        capability_provider::ICapabilityProvider,
        gpu_controller::{GpuControlStatus, IGpuController},
        hardware_executor::{
            HardwareExecutionHint, HardwareExecutionRequest, HardwareExecutionResult,
            HardwareExecutionStatus, IHardwareExecutor,
        },
        hardware_telemetry::IHardwareTelemetryProvider,
        npu_controller::{INpuController, NpuControlStatus},
        power_controller::IPowerController,
        process_controller::{AffinityMask, IProcessController},
        safe_actuator::{ActuationPermit, ISafeHardwareActuator},
        telemetry_provider::ITelemetryProvider,
    },
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessState {
    pub priority: PolicyPriority,
    pub affinity_cores: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformFixture {
    pub source: String,
    pub snapshot: ComputerSnapshot,
    pub capabilities: Vec<CapabilityEntry>,
    pub processes: BTreeMap<u32, ProcessState>,
    pub power_profile: Option<PowerMode>,
    pub gpu_samples: Vec<GpuTelemetrySample>,
    pub npu_samples: Vec<NpuTelemetrySample>,
    pub hardware_matrices: Vec<DeviceCapabilityMatrix>,
}

#[derive(Debug)]
struct State {
    processes: BTreeMap<u32, ProcessState>,
    power_profile: Option<PowerMode>,
    hardware_hints: BTreeMap<String, HardwareExecutionHint>,
    fail_next_process_write: bool,
    stale_after_write: bool,
    stale_read: Option<(u32, PolicyPriority)>,
}

#[derive(Debug, Clone)]
pub struct SimulatedPlatform {
    fixture: Arc<PlatformFixture>,
    state: Arc<Mutex<State>>,
}

impl SimulatedPlatform {
    pub fn from_fixture(fixture: PlatformFixture) -> ContractResult<Self> {
        let gpu_ids: BTreeSet<_> = fixture
            .gpu_samples
            .iter()
            .map(|sample| sample.device.id.as_str())
            .collect();
        let npu_ids: BTreeSet<_> = fixture
            .npu_samples
            .iter()
            .map(|sample| sample.device.id.as_str())
            .collect();
        if fixture.source.trim().is_empty()
            || fixture.snapshot.collected_at_unix_ms.is_none()
            || fixture.capabilities.iter().any(|cap| !cap.is_well_formed())
            || fixture.processes.keys().any(|pid| {
                !fixture
                    .snapshot
                    .top_processes
                    .iter()
                    .any(|process| process.pid == *pid)
            })
            || fixture.hardware_matrices.iter().any(|m| {
                m.device_id != m.capabilities.device_name
                    || match m.capabilities.device_kind {
                        HardwareDeviceKind::Gpu => !gpu_ids.contains(m.device_id.as_str()),
                        HardwareDeviceKind::Npu => !npu_ids.contains(m.device_id.as_str()),
                        HardwareDeviceKind::Unknown => true,
                    }
            })
        {
            return Err(ContractError::ContractInvalid(
                "invalid simulator fixture provenance or capability identity".into(),
            ));
        }
        let state = State {
            processes: fixture.processes.clone(),
            power_profile: fixture.power_profile,
            hardware_hints: BTreeMap::new(),
            fail_next_process_write: false,
            stale_after_write: false,
            stale_read: None,
        };
        Ok(Self {
            fixture: Arc::new(fixture),
            state: Arc::new(Mutex::new(state)),
        })
    }
    pub fn from_json(json: &str) -> ContractResult<Self> {
        let fixture = serde_json::from_str(json).map_err(|e| {
            ContractError::ContractInvalid(format!("invalid platform fixture: {e}"))
        })?;
        Self::from_fixture(fixture)
    }
    pub fn hardware_matrices(&self) -> &[DeviceCapabilityMatrix] {
        &self.fixture.hardware_matrices
    }
    pub fn captured_process_ids(&self) -> Vec<u32> {
        self.fixture.processes.keys().copied().collect()
    }
    pub fn fail_next_process_write(&self) -> ContractResult<()> {
        self.lock()?.fail_next_process_write = true;
        Ok(())
    }
    pub fn stale_read_after_next_write(&self) -> ContractResult<()> {
        self.lock()?.stale_after_write = true;
        Ok(())
    }
    pub fn hardware_hint(&self, id: &str) -> ContractResult<Option<HardwareExecutionHint>> {
        Ok(self.lock()?.hardware_hints.get(id).cloned())
    }
    fn lock(&self) -> ContractResult<std::sync::MutexGuard<'_, State>> {
        self.state
            .lock()
            .map_err(|_| ContractError::LockPoisoned("simulated platform".into()))
    }
    fn capability(&self, id: &str) -> Option<&HardwareCapabilitySet> {
        self.fixture
            .hardware_matrices
            .iter()
            .find(|m| m.device_id == id)
            .map(|m| &m.capabilities)
    }
    fn supports(&self, id: &str, cap: HardwareCapability) -> bool {
        self.capability(id)
            .map(|set| set.is_supported(cap))
            .unwrap_or(false)
    }
}

impl ITelemetryProvider for SimulatedPlatform {
    fn collect(&self) -> ContractResult<ComputerSnapshot> {
        let mut snapshot = self.fixture.snapshot.clone();
        if let Some(mode) = self.lock()?.power_profile {
            snapshot.power.power_plan = Some(format!("{mode:?}"));
        }
        Ok(snapshot)
    }
    fn platform_name(&self) -> &'static str {
        "simulated_platform"
    }
    fn capabilities(&self) -> Vec<&'static str> {
        vec!["cpu.telemetry", "memory.telemetry", "process.telemetry"]
    }
}
impl ICapabilityProvider for SimulatedPlatform {
    fn capabilities_detailed(&self) -> Vec<CapabilityEntry> {
        self.fixture
            .capabilities
            .iter()
            .cloned()
            .map(|mut cap| {
                if cap.name.ends_with(".write")
                    || cap.name.ends_with(".control")
                    || cap.name == "power.profile"
                {
                    cap.detail = Some(match cap.detail {
                        Some(detail) => format!("{detail}; simulator writes memory only"),
                        None => "simulator writes memory only".into(),
                    });
                }
                cap
            })
            .collect()
    }
    fn component_name(&self) -> &'static str {
        "simulated_platform"
    }
}
impl IProcessController for SimulatedPlatform {
    fn set_priority(&self, pid: u32, priority: PolicyPriority) -> ContractResult<()> {
        let mut state = self.lock()?;
        if state.fail_next_process_write {
            state.fail_next_process_write = false;
            return Err(ContractError::AdapterUnavailable(
                "injected simulator process failure".into(),
            ));
        }
        let process = state.processes.get_mut(&pid).ok_or_else(|| {
            ContractError::CapabilityUnavailable(format!("PID {pid} absent from fixture"))
        })?;
        let previous = process.priority;
        process.priority = priority;
        if state.stale_after_write {
            state.stale_after_write = false;
            state.stale_read = Some((pid, previous));
        }
        Ok(())
    }
    fn get_priority(&self, pid: u32) -> ContractResult<PolicyPriority> {
        let mut state = self.lock()?;
        if state.stale_read.as_ref().map(|(id, _)| *id) == Some(pid) {
            if let Some((_, priority)) = state.stale_read.take() {
                return Ok(priority);
            }
        }
        state
            .processes
            .get(&pid)
            .map(|p| p.priority)
            .ok_or_else(|| {
                ContractError::CapabilityUnavailable(format!("PID {pid} absent from fixture"))
            })
    }
    fn set_affinity(&self, pid: u32, mask: &AffinityMask) -> ContractResult<()> {
        if mask.is_empty() {
            return Err(ContractError::InvalidRequest("empty affinity mask".into()));
        }
        let mut state = self.lock()?;
        let allowed = self.fixture.processes.get(&pid).ok_or_else(|| {
            ContractError::CapabilityUnavailable(format!("PID {pid} absent from fixture"))
        })?;
        if mask
            .cores
            .iter()
            .any(|core| !allowed.affinity_cores.contains(core))
        {
            return Err(ContractError::CapabilityUnavailable(
                "core outside captured affinity".into(),
            ));
        }
        let process = state.processes.get_mut(&pid).ok_or_else(|| {
            ContractError::CapabilityUnavailable(format!("PID {pid} absent from fixture"))
        })?;
        process.affinity_cores = mask.cores.clone();
        Ok(())
    }
    fn get_affinity(&self, pid: u32) -> ContractResult<AffinityMask> {
        self.lock()?
            .processes
            .get(&pid)
            .map(|p| AffinityMask::new(p.affinity_cores.clone()))
            .ok_or_else(|| {
                ContractError::CapabilityUnavailable(format!("PID {pid} absent from fixture"))
            })
    }
    fn capabilities(&self) -> Vec<&'static str> {
        if self.fixture.processes.is_empty() {
            Vec::new()
        } else {
            vec![
                "process.priority.read",
                "process.priority.write",
                "process.affinity.read",
                "process.affinity.write",
            ]
        }
    }
    fn platform_name(&self) -> &'static str {
        "simulated_platform"
    }
}
impl IPowerController for SimulatedPlatform {
    fn set_profile(&self, mode: PowerMode) -> ContractResult<PowerMode> {
        let mut state = self.lock()?;
        let profile = state.power_profile.as_mut().ok_or_else(|| {
            ContractError::CapabilityUnavailable("power profile absent from fixture".into())
        })?;
        *profile = mode;
        Ok(mode)
    }
    fn get_profile(&self) -> ContractResult<PowerMode> {
        self.lock()?.power_profile.ok_or_else(|| {
            ContractError::CapabilityUnavailable("power profile absent from fixture".into())
        })
    }
    fn capabilities(&self) -> Vec<&'static str> {
        if self.fixture.power_profile.is_some() {
            vec!["power.profile.read", "power.profile.write"]
        } else {
            Vec::new()
        }
    }
    fn platform_name(&self) -> &'static str {
        "simulated_platform"
    }
}
impl IGpuController for SimulatedPlatform {
    fn set_workload_hint(&self, hint: &str) -> ContractResult<()> {
        let sample = self
            .fixture
            .gpu_samples
            .iter()
            .find(|s| self.supports(&s.device.id, HardwareCapability::WorkloadHint))
            .ok_or_else(|| {
                ContractError::CapabilityUnavailable("GPU control absent from fixture".into())
            })?;
        let result = self.apply(&HardwareExecutionRequest {
            device_id: sample.device.id.clone(),
            hint: HardwareExecutionHint::WorkloadPreference(hint.into()),
        })?;
        if result.status.is_applied() {
            Ok(())
        } else {
            Err(ContractError::CapabilityUnavailable(format!(
                "{:?}",
                result.status
            )))
        }
    }
    fn get_status(&self) -> ContractResult<GpuControlStatus> {
        Ok(GpuControlStatus {
            controllable: self
                .fixture
                .gpu_samples
                .iter()
                .any(|s| self.supports(&s.device.id, HardwareCapability::WorkloadHint)),
            detail: Some("captured capability; memory-only simulator".into()),
        })
    }
    fn capabilities(&self) -> Vec<&'static str> {
        vec!["gpu.status"]
    }
    fn platform_name(&self) -> &'static str {
        "simulated_platform"
    }
}
impl INpuController for SimulatedPlatform {
    fn submit_inference_hint(&self, hint: &str) -> ContractResult<()> {
        let sample = self
            .fixture
            .npu_samples
            .iter()
            .find(|s| self.supports(&s.device.id, HardwareCapability::WorkloadHint))
            .ok_or_else(|| {
                ContractError::CapabilityUnavailable("NPU control absent from fixture".into())
            })?;
        let result = self.apply(&HardwareExecutionRequest {
            device_id: sample.device.id.clone(),
            hint: HardwareExecutionHint::WorkloadPreference(hint.into()),
        })?;
        if result.status.is_applied() {
            Ok(())
        } else {
            Err(ContractError::CapabilityUnavailable(format!(
                "{:?}",
                result.status
            )))
        }
    }
    fn get_status(&self) -> ContractResult<NpuControlStatus> {
        Ok(NpuControlStatus {
            controllable: self
                .fixture
                .npu_samples
                .iter()
                .any(|s| self.supports(&s.device.id, HardwareCapability::WorkloadHint)),
            detail: Some("captured capability; memory-only simulator".into()),
        })
    }
    fn capabilities(&self) -> Vec<&'static str> {
        vec!["npu.status"]
    }
    fn platform_name(&self) -> &'static str {
        "simulated_platform"
    }
}
impl IHardwareTelemetryProvider for SimulatedPlatform {
    fn gpu_devices(&self) -> Vec<HardwareDeviceInfo> {
        self.fixture
            .gpu_samples
            .iter()
            .map(|s| s.device.clone())
            .collect()
    }
    fn npu_devices(&self) -> Vec<HardwareDeviceInfo> {
        self.fixture
            .npu_samples
            .iter()
            .map(|s| s.device.clone())
            .collect()
    }
    fn sample_gpu(&self, id: &str) -> ContractResult<GpuTelemetrySample> {
        self.fixture
            .gpu_samples
            .iter()
            .find(|s| s.device.id == id)
            .cloned()
            .ok_or_else(|| {
                ContractError::CapabilityUnavailable(format!("GPU {id} absent from fixture"))
            })
    }
    fn sample_npu(&self, id: &str) -> ContractResult<NpuTelemetrySample> {
        self.fixture
            .npu_samples
            .iter()
            .find(|s| s.device.id == id)
            .cloned()
            .ok_or_else(|| {
                ContractError::CapabilityUnavailable(format!("NPU {id} absent from fixture"))
            })
    }
    fn provider_name(&self) -> &'static str {
        "simulated_platform"
    }
}
impl IHardwareExecutor for SimulatedPlatform {
    fn apply(&self, request: &HardwareExecutionRequest) -> ContractResult<HardwareExecutionResult> {
        let needed = match request.hint {
            HardwareExecutionHint::MaxPowerWatts(_) => HardwareCapability::VendorControl,
            _ => HardwareCapability::WorkloadHint,
        };
        let status = if !self.supports(&request.device_id, needed) {
            HardwareExecutionStatus::NotSupported {
                reason: format!("{} absent from captured capability", needed.name()),
            }
        } else {
            let mut state = self.lock()?;
            if state.hardware_hints.get(&request.device_id) == Some(&request.hint) {
                HardwareExecutionStatus::AlreadyInState
            } else {
                state
                    .hardware_hints
                    .insert(request.device_id.clone(), request.hint.clone());
                HardwareExecutionStatus::Applied
            }
        };
        Ok(HardwareExecutionResult {
            device_id: request.device_id.clone(),
            status,
            latency_ms: 0,
        })
    }
    fn release(&self, id: &str) -> ContractResult<HardwareExecutionResult> {
        let status = if !self.supports(id, HardwareCapability::WorkloadHint)
            && !self.supports(id, HardwareCapability::VendorControl)
        {
            HardwareExecutionStatus::NotSupported {
                reason: "captured device has no control capability".into(),
            }
        } else if self.lock()?.hardware_hints.remove(id).is_some() {
            HardwareExecutionStatus::Applied
        } else {
            HardwareExecutionStatus::AlreadyInState
        };
        Ok(HardwareExecutionResult {
            device_id: id.into(),
            status,
            latency_ms: 0,
        })
    }
    fn executor_name(&self) -> &'static str {
        "simulated_platform"
    }
}
impl ISafeHardwareActuator for SimulatedPlatform {
    fn apply_authorized(
        &self,
        request: &HardwareExecutionRequest,
        permit: &ActuationPermit,
        now: u64,
    ) -> ContractResult<HardwareExecutionResult> {
        let needed = match request.hint {
            HardwareExecutionHint::MaxPowerWatts(_) => HardwareCapability::VendorControl,
            _ => HardwareCapability::WorkloadHint,
        };
        if !permit.model_only
            || permit.device_id != request.device_id
            || permit.expires_at_unix_ms <= now
            || permit.capability != needed
        {
            return Err(ContractError::PermissionDenied(
                "invalid simulator actuation permit".into(),
            ));
        }
        self.apply(request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn captured() -> SimulatedPlatform {
        SimulatedPlatform::from_json(include_str!("../fixtures/linux_wsl_readonly.json"))
            .expect("bundled read-only capture must be valid")
    }

    #[test]
    fn captured_process_affinity_can_be_restored_in_memory() {
        let platform = captured();
        let pid = platform.captured_process_ids()[0];
        let original = platform.get_affinity(pid).unwrap();
        let subset = AffinityMask::new(vec![original.cores[0]]);
        platform.set_affinity(pid, &subset).unwrap();
        assert_eq!(platform.get_affinity(pid).unwrap(), subset);
        platform.set_affinity(pid, &original).unwrap();
        assert_eq!(platform.get_affinity(pid).unwrap(), original);
    }

    #[test]
    fn absent_devices_and_power_are_never_reported_as_applied() {
        let platform = captured();
        assert!(platform.gpu_devices().is_empty());
        assert!(platform.npu_devices().is_empty());
        assert!(!IGpuController::get_status(&platform).unwrap().controllable);
        assert!(!INpuController::get_status(&platform).unwrap().controllable);
        assert!(IGpuController::set_workload_hint(&platform, "balanced").is_err());
        assert!(INpuController::submit_inference_hint(&platform, "balanced").is_err());
        let request = HardwareExecutionRequest {
            device_id: "not-in-capture".into(),
            hint: HardwareExecutionHint::AutoMode,
        };
        assert!(matches!(
            platform.apply(&request).unwrap().status,
            HardwareExecutionStatus::NotSupported { .. }
        ));
        assert!(matches!(
            platform.release(&request.device_id).unwrap().status,
            HardwareExecutionStatus::NotSupported { .. }
        ));
        assert!(platform.get_profile().is_err());
        let invalid_permit = ActuationPermit {
            device_id: request.device_id.clone(),
            capability: HardwareCapability::VendorControl,
            expires_at_unix_ms: 100,
            model_only: true,
        };
        assert!(platform
            .apply_authorized(&request, &invalid_permit, 1)
            .is_err());
    }

    #[test]
    fn fixture_cannot_add_process_not_present_in_recorded_snapshot() {
        let mut fixture: PlatformFixture =
            serde_json::from_str(include_str!("../fixtures/linux_wsl_readonly.json")).unwrap();
        fixture.processes.insert(
            u32::MAX,
            ProcessState {
                priority: PolicyPriority::Normal,
                affinity_cores: vec![0],
            },
        );
        assert!(SimulatedPlatform::from_fixture(fixture).is_err());
    }
}
