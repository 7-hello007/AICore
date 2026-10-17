use aicore_contracts::{
    errors::{ContractError, ContractResult},
    hardware::{
        capability::{HardwareCapability, HardwareDeviceKind},
        matrix::DeviceCapabilityMatrix,
    },
    policy::priority::PolicyPriority,
    traits::{
        hardware_executor::{
            HardwareExecutionHint, HardwareExecutionRequest, HardwareExecutionResult,
        },
        safe_actuator::{ActuationPermit, ISafeHardwareActuator},
    },
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[derive(Debug, Clone)]
pub struct AcceleratorJob {
    pub id: String,
    pub kind: HardwareDeviceKind,
    pub priority: PolicyPriority,
    pub expires_at_unix_ms: u64,
    pub hint: HardwareExecutionHint,
}
struct Device {
    matrix: DeviceCapabilityMatrix,
    slots: usize,
    used: usize,
    temperature: Option<f32>,
}
#[derive(Default)]
struct State {
    devices: Vec<Device>,
    waiting: Vec<(u64, AcceleratorJob)>,
    active: HashMap<String, String>,
    next: u64,
}

/// Bounded, deterministic admission and placement. Slots are backend-provided
/// limits, not inferred from vendor names. Reservation is memory-only; native
/// dispatch remains gated by an independent safe actuator and live permit.
pub struct AcceleratorScheduler {
    state: Arc<Mutex<State>>,
    capacity: usize,
}
pub struct AcceleratorLease {
    state: Arc<Mutex<State>>,
    job: AcceleratorJob,
    device_id: String,
    dispatched: std::sync::atomic::AtomicBool,
}

fn required(hint: &HardwareExecutionHint) -> HardwareCapability {
    match hint {
        HardwareExecutionHint::WorkloadPreference(_) => HardwareCapability::WorkloadHint,
        HardwareExecutionHint::MaxPowerWatts(_) => HardwareCapability::VendorControl,
        _ => HardwareCapability::ProviderSelection,
    }
}
impl AcceleratorScheduler {
    pub fn new(capacity: usize) -> ContractResult<Self> {
        if capacity == 0 || capacity > 4096 {
            return Err(ContractError::ContractInvalid(
                "invalid accelerator queue capacity".into(),
            ));
        }
        Ok(Self {
            state: Arc::new(Mutex::new(State::default())),
            capacity,
        })
    }
    pub fn register_device(
        &self,
        matrix: DeviceCapabilityMatrix,
        concurrent_slots: usize,
    ) -> ContractResult<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ContractError::LockPoisoned("scheduler poisoned".into()))?;
        if concurrent_slots == 0
            || concurrent_slots > 64
            || matrix.device_id.is_empty()
            || matrix.capabilities.device_name != matrix.device_id
            || matrix.capabilities.device_kind == HardwareDeviceKind::Unknown
            || state
                .devices
                .iter()
                .any(|d| d.matrix.device_id == matrix.device_id)
        {
            return Err(ContractError::ContractInvalid(
                "invalid accelerator registration".into(),
            ));
        }
        state.devices.push(Device {
            matrix,
            slots: concurrent_slots,
            used: 0,
            temperature: None,
        });
        Ok(())
    }
    pub fn update_temperature(
        &self,
        device_id: &str,
        temperature: Option<f32>,
    ) -> ContractResult<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ContractError::LockPoisoned("scheduler poisoned".into()))?;
        let device = state
            .devices
            .iter_mut()
            .find(|d| d.matrix.device_id == device_id)
            .ok_or_else(|| ContractError::ResourceUnavailable("device not registered".into()))?;
        device.temperature = temperature.filter(|v| v.is_finite());
        Ok(())
    }
    pub fn enqueue(&self, job: AcceleratorJob, now_unix_ms: u64) -> ContractResult<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ContractError::LockPoisoned("scheduler poisoned".into()))?;
        let valid_hint = match &job.hint {
            HardwareExecutionHint::MaxPowerWatts(v) => v.is_finite() && *v > 0.0,
            HardwareExecutionHint::WorkloadPreference(v) => {
                matches!(v.as_str(), "graphics" | "compute" | "auto")
            },
            _ => true,
        };
        if job.id.trim().is_empty()
            || job.id.len() > 256
            || job.expires_at_unix_ms <= now_unix_ms
            || job.kind == HardwareDeviceKind::Unknown
            || !valid_hint
        {
            return Err(ContractError::InvalidRequest(
                "invalid/expired accelerator job".into(),
            ));
        }
        if state.waiting.len() + state.active.len() >= self.capacity {
            return Err(ContractError::ResourceUnavailable(
                "accelerator queue full".into(),
            ));
        }
        if state.active.contains_key(&job.id) || state.waiting.iter().any(|(_, j)| j.id == job.id) {
            return Err(ContractError::InvalidRequest(
                "duplicate accelerator job".into(),
            ));
        }
        let seq = state.next;
        state.next = state.next.checked_add(1).ok_or_else(|| {
            ContractError::ResourceUnavailable("scheduler sequence exhausted".into())
        })?;
        state.waiting.push((seq, job));
        Ok(())
    }
    pub fn cancel(&self, id: &str) -> ContractResult<bool> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ContractError::LockPoisoned("scheduler poisoned".into()))?;
        if state.active.contains_key(id) {
            return Err(ContractError::ResourceUnavailable(
                "leased work must finish/release before cancellation".into(),
            ));
        }
        let old = state.waiting.len();
        state.waiting.retain(|(_, j)| j.id != id);
        Ok(old != state.waiting.len())
    }
    pub fn reserve(&self, now_unix_ms: u64) -> ContractResult<Option<AcceleratorLease>> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ContractError::LockPoisoned("scheduler poisoned".into()))?;
        state
            .waiting
            .retain(|(_, j)| j.expires_at_unix_ms > now_unix_ms);
        state.waiting.sort_by(|(a, x), (b, y)| {
            y.priority
                .cmp(&x.priority)
                .then(x.expires_at_unix_ms.cmp(&y.expires_at_unix_ms))
                .then(a.cmp(b))
        });
        let selected = state.waiting.iter().enumerate().find_map(|(i, (_, job))| {
            state
                .devices
                .iter()
                .position(|d| {
                    let thermal_ok = d
                        .matrix
                        .thermal_limit_celsius
                        .map(|limit| limit.is_finite() && d.temperature.is_some_and(|t| t < limit))
                        .unwrap_or(true);
                    d.used < d.slots
                        && d.matrix.capabilities.device_kind == job.kind
                        && d.matrix.supports(required(&job.hint))
                        && thermal_ok
                })
                .map(|device| (i, device))
        });
        let Some((index, device)) = selected else {
            return Ok(None);
        };
        let (_, job) = state.waiting.remove(index);
        state.devices[device].used += 1;
        let device_id = state.devices[device].matrix.device_id.clone();
        state.active.insert(job.id.clone(), device_id.clone());
        Ok(Some(AcceleratorLease {
            state: self.state.clone(),
            job,
            device_id,
            dispatched: std::sync::atomic::AtomicBool::new(false),
        }))
    }
}
impl AcceleratorLease {
    pub fn job(&self) -> &AcceleratorJob {
        &self.job
    }
    pub fn device_id(&self) -> &str {
        &self.device_id
    }
    pub fn apply(
        &self,
        actuator: &dyn ISafeHardwareActuator,
        permit: &ActuationPermit,
        now_unix_ms: u64,
    ) -> ContractResult<HardwareExecutionResult> {
        if now_unix_ms >= self.job.expires_at_unix_ms
            || permit.device_id != self.device_id
            || permit.capability != required(&self.job.hint)
            || now_unix_ms >= permit.expires_at_unix_ms
        {
            return Err(ContractError::PermissionDenied(
                "accelerator lease expired or permit mismatched".into(),
            ));
        }
        {
            let state = self
                .state
                .lock()
                .map_err(|_| ContractError::LockPoisoned("scheduler poisoned".into()))?;
            let device = state
                .devices
                .iter()
                .find(|d| d.matrix.device_id == self.device_id)
                .ok_or_else(|| {
                    ContractError::ResourceUnavailable("leased device disappeared".into())
                })?;
            if !device.matrix.supports(required(&self.job.hint))
                || device.matrix.thermal_limit_celsius.is_some_and(|limit| {
                    !limit.is_finite() || !device.temperature.is_some_and(|t| t < limit)
                })
            {
                return Err(ContractError::CapabilityUnavailable(
                    "leased device capability/thermal guard unavailable".into(),
                ));
            }
        }
        if self
            .dispatched
            .swap(true, std::sync::atomic::Ordering::AcqRel)
        {
            return Err(ContractError::InvalidRequest(
                "accelerator lease already dispatched".into(),
            ));
        }
        actuator.apply_authorized(
            &HardwareExecutionRequest {
                device_id: self.device_id.clone(),
                hint: self.job.hint.clone(),
            },
            permit,
            now_unix_ms,
        )
    }
}
impl Drop for AcceleratorLease {
    fn drop(&mut self) {
        match self.state.lock() {
            Ok(mut state) => {
                state.active.remove(&self.job.id);
                if let Some(device) = state
                    .devices
                    .iter_mut()
                    .find(|d| d.matrix.device_id == self.device_id)
                {
                    device.used = device.used.saturating_sub(1);
                }
            },
            Err(error) => tracing::error!(%error,"accelerator lease release failed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aicore_contracts::hardware::{
        capability::{HardwareCapabilityEntry, HardwareCapabilitySet},
        matrix::{HardwareVendor, HostOs},
    };
    #[test]
    fn capacity_deadline_thermal_cancel_and_lease_cleanup() {
        let scheduler = AcceleratorScheduler::new(3).unwrap();
        let mut caps =
            HardwareCapabilitySet::new("gpu:test").with_device_kind(HardwareDeviceKind::Gpu);
        caps.entries.push(HardwareCapabilityEntry::supported(
            HardwareCapability::WorkloadHint,
        ));
        scheduler
            .register_device(
                DeviceCapabilityMatrix {
                    os: HostOs::Windows,
                    vendor: HardwareVendor::Other("mock".into()),
                    device_id: "gpu:test".into(),
                    capabilities: caps,
                    thermal_limit_celsius: Some(85.0),
                },
                1,
            )
            .unwrap();
        let job = |id: &str, priority| AcceleratorJob {
            id: id.into(),
            kind: HardwareDeviceKind::Gpu,
            priority,
            expires_at_unix_ms: 100,
            hint: HardwareExecutionHint::WorkloadPreference("compute".into()),
        };
        scheduler
            .enqueue(job("low", PolicyPriority::Low), 1)
            .unwrap();
        scheduler
            .enqueue(job("high", PolicyPriority::High), 1)
            .unwrap();
        assert!(scheduler.reserve(2).unwrap().is_none());
        scheduler
            .update_temperature("gpu:test", Some(50.0))
            .unwrap();
        let lease = scheduler.reserve(2).unwrap().unwrap();
        assert_eq!(lease.job.id, "high");
        struct MockActuator;
        impl ISafeHardwareActuator for MockActuator {
            fn apply_authorized(
                &self,
                r: &HardwareExecutionRequest,
                _: &ActuationPermit,
                _: u64,
            ) -> ContractResult<HardwareExecutionResult> {
                Ok(HardwareExecutionResult{device_id:r.device_id.clone(),status:aicore_contracts::traits::hardware_executor::HardwareExecutionStatus::Applied,latency_ms:0})
            }
        }
        let permit = ActuationPermit {
            device_id: "gpu:test".into(),
            capability: HardwareCapability::WorkloadHint,
            expires_at_unix_ms: 100,
            model_only: false,
        };
        scheduler
            .update_temperature("gpu:test", Some(90.0))
            .unwrap();
        assert!(lease.apply(&MockActuator, &permit, 2).is_err());
        scheduler
            .update_temperature("gpu:test", Some(50.0))
            .unwrap();
        assert!(lease.apply(&MockActuator, &permit, 2).is_ok());
        assert!(lease.apply(&MockActuator, &permit, 2).is_err());
        assert!(scheduler.reserve(2).unwrap().is_none());
        assert!(scheduler.cancel("high").is_err());
        drop(lease);
        let next = scheduler.reserve(2).unwrap().unwrap();
        assert_eq!(next.job.id, "low");
        drop(next);
        scheduler
            .enqueue(job("cancel", PolicyPriority::Low), 1)
            .unwrap();
        assert!(scheduler.cancel("cancel").unwrap());
        scheduler
            .enqueue(job("expired", PolicyPriority::Low), 1)
            .unwrap();
        assert!(scheduler.reserve(101).unwrap().is_none());
    }
}
