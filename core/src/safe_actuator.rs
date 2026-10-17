use std::sync::{Arc, Mutex};
use std::time::Instant;

use aicore_contracts::{
    errors::ContractResult,
    hardware::{capability::HardwareCapability, matrix::DeviceCapabilityMatrix},
    traits::{
        hardware_executor::{
            HardwareExecutionHint, HardwareExecutionRequest, HardwareExecutionResult,
            HardwareExecutionStatus, IHardwareExecutor,
        },
        safe_actuator::{ActuationPermit, ISafeHardwareActuator},
    },
};

// Includes distinct wrappers sharing a backend, not only one actuator instance.
static HARDWARE_TRANSACTION: Mutex<()> = Mutex::new(());

pub struct GuardedHardwareActuator {
    matrix: DeviceCapabilityMatrix,
    backend: Option<Arc<dyn IHardwareExecutor>>,
}

impl GuardedHardwareActuator {
    pub fn new(
        matrix: DeviceCapabilityMatrix,
        backend: Option<Arc<dyn IHardwareExecutor>>,
    ) -> Self {
        Self { matrix, backend }
    }
}

impl ISafeHardwareActuator for GuardedHardwareActuator {
    fn apply_authorized(
        &self,
        request: &HardwareExecutionRequest,
        permit: &ActuationPermit,
        now_unix_ms: u64,
    ) -> ContractResult<HardwareExecutionResult> {
        let started = Instant::now();
        let result = |status| HardwareExecutionResult {
            device_id: request.device_id.clone(),
            status,
            latency_ms: started.elapsed().as_millis() as u64,
        };
        let rejected = |reason: &str| {
            result(HardwareExecutionStatus::NotSupported {
                reason: reason.into(),
            })
        };
        let required = match &request.hint {
            HardwareExecutionHint::WorkloadPreference(value)
                if value.trim().is_empty() || value.len() > 64 =>
            {
                return Ok(rejected("invalid workload hint"))
            },
            HardwareExecutionHint::WorkloadPreference(_) => HardwareCapability::WorkloadHint,
            HardwareExecutionHint::MaxPowerWatts(value) if !value.is_finite() || *value <= 0.0 => {
                return Ok(rejected("invalid power limit"))
            },
            HardwareExecutionHint::MaxPowerWatts(_) => HardwareCapability::VendorControl,
            HardwareExecutionHint::GraphicsMode
            | HardwareExecutionHint::ComputeMode
            | HardwareExecutionHint::AutoMode => HardwareCapability::ProviderSelection,
        };
        if request.device_id.is_empty()
            || request.device_id != self.matrix.device_id
            || self.matrix.capabilities.device_name != request.device_id
            || self.matrix.capabilities.device_kind
                == aicore_contracts::hardware::capability::HardwareDeviceKind::Unknown
            || !self.matrix.supports(HardwareCapability::DeviceIdentity)
            || permit.device_id != request.device_id
            || permit.capability != required
            || now_unix_ms >= permit.expires_at_unix_ms
            || !self.matrix.supports(required)
        {
            return Ok(rejected("device, capability or permit mismatch/expiry"));
        }
        let backend = match &self.backend {
            Some(backend) => backend,
            None => return Ok(rejected("no actuator backend bound")),
        };
        if permit.model_only != backend.is_model_only() {
            return Ok(result(HardwareExecutionStatus::Skipped {
                reason: "permit and backend model-only domains differ".into(),
            }));
        }
        if !backend.supports_reversible_state() {
            return Ok(rejected(
                "exact independently readable restoration unavailable",
            ));
        }
        let _transaction = HARDWARE_TRANSACTION.lock().map_err(|_| {
            aicore_contracts::errors::ContractError::AdapterUnavailable(
                "hardware transaction lock poisoned".into(),
            )
        })?;
        let original = match backend.read_hardware_state(&request.device_id) {
            Ok(state) if state.device_id == request.device_id => state,
            _ => return Ok(rejected("independent original hardware state unavailable")),
        };
        if now_unix_ms.saturating_add(started.elapsed().as_millis() as u64)
            >= permit.expires_at_unix_ms
        {
            return Ok(rejected("permit expired before hardware write"));
        }
        let applied = backend.apply(request);
        if let Ok(applied) = &applied {
            if applied.device_id == request.device_id && applied.status.is_applied() {
                if let Ok(observed) = backend.read_hardware_state(&request.device_id) {
                    if observed.device_id == request.device_id
                        && observed.hint.as_ref() == Some(&request.hint)
                        && now_unix_ms.saturating_add(started.elapsed().as_millis() as u64)
                            < permit.expires_at_unix_ms
                    {
                        let status = if backend.is_model_only() {
                            if matches!(applied.status, HardwareExecutionStatus::AlreadyInState) {
                                HardwareExecutionStatus::AlreadyInState
                            } else {
                                HardwareExecutionStatus::Applied
                            }
                        } else {
                            HardwareExecutionStatus::Verified
                        };
                        return Ok(result(status));
                    }
                }
            }
        }
        // A command can fail after partial mutation; always restore and read back.
        let restored = backend.restore_hardware_state(&original);
        let final_state = backend.read_hardware_state(&request.device_id);
        let restoration_verified = restored
            .as_ref()
            .map(|r| r.device_id == request.device_id && r.status.is_applied())
            .unwrap_or(false)
            && final_state.map(|state| state == original).unwrap_or(false);
        if restoration_verified {
            if let Ok(outcome) = &applied {
                if outcome.device_id == request.device_id
                    && matches!(
                        outcome.status,
                        HardwareExecutionStatus::NotSupported { .. }
                            | HardwareExecutionStatus::Skipped { .. }
                    )
                {
                    return Ok(result(outcome.status.clone()));
                }
            }
        }
        Ok(result(HardwareExecutionStatus::Failed {
            reason: if restoration_verified {
                format!(
                    "hardware apply/readback failed; exact original restored: {:?}",
                    applied
                )
            } else {
                format!(
                    "hardware apply/readback failed; restoration NOT VERIFIED: {:?}; {:?}",
                    applied, restored
                )
            },
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aicore_contracts::hardware::{
        capability::{HardwareCapabilityEntry, HardwareCapabilitySet},
        matrix::{HardwareVendor, HostOs},
    };

    struct MockBackend;
    impl IHardwareExecutor for MockBackend {
        fn apply(
            &self,
            request: &HardwareExecutionRequest,
        ) -> ContractResult<HardwareExecutionResult> {
            Ok(HardwareExecutionResult {
                device_id: request.device_id.clone(),
                status: HardwareExecutionStatus::Applied,
                latency_ms: 1,
            })
        }
        fn release(&self, _device_id: &str) -> ContractResult<HardwareExecutionResult> {
            panic!("release not expected")
        }
        fn executor_name(&self) -> &'static str {
            "mock"
        }
    }
    fn actuator() -> GuardedHardwareActuator {
        GuardedHardwareActuator::new(
            DeviceCapabilityMatrix {
                os: HostOs::Linux,
                vendor: HardwareVendor::Other("mock".into()),
                device_id: "gpu:mock".into(),
                capabilities: HardwareCapabilitySet::new("gpu:mock")
                    .with_device_kind(
                        aicore_contracts::hardware::capability::HardwareDeviceKind::Gpu,
                    )
                    .with(HardwareCapabilityEntry::supported(
                        HardwareCapability::DeviceIdentity,
                    ))
                    .with(HardwareCapabilityEntry::supported(
                        HardwareCapability::WorkloadHint,
                    )),
                thermal_limit_celsius: None,
            },
            Some(Arc::new(MockBackend)),
        )
    }
    #[test]
    fn legacy_ack_only_backend_requires_reversible_state() {
        let request = HardwareExecutionRequest {
            device_id: "gpu:mock".into(),
            hint: HardwareExecutionHint::WorkloadPreference("compute".into()),
        };
        let mut permit = ActuationPermit {
            device_id: "gpu:mock".into(),
            capability: HardwareCapability::WorkloadHint,
            expires_at_unix_ms: 100,
            model_only: true,
        };
        assert!(matches!(
            actuator()
                .apply_authorized(&request, &permit, 10)
                .unwrap()
                .status,
            HardwareExecutionStatus::Skipped { .. }
        ));
        permit.model_only = false;
        assert!(matches!(
            actuator()
                .apply_authorized(&request, &permit, 100)
                .unwrap()
                .status,
            HardwareExecutionStatus::NotSupported { .. }
        ));
        assert!(matches!(
            actuator()
                .apply_authorized(&request, &permit, 10)
                .unwrap()
                .status,
            HardwareExecutionStatus::NotSupported { .. }
        ));
    }
}
#[cfg(test)]
mod p0_tests {
    use super::*;
    use aicore_contracts::hardware::capability::{HardwareCapabilityEntry, HardwareCapabilitySet};
    use aicore_contracts::hardware::matrix::{HardwareVendor, HostOs};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct AckOnly(Arc<AtomicUsize>);
    impl IHardwareExecutor for AckOnly {
        fn apply(&self, req: &HardwareExecutionRequest) -> ContractResult<HardwareExecutionResult> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(HardwareExecutionResult {
                device_id: req.device_id.clone(),
                status: HardwareExecutionStatus::Applied,
                latency_ms: 0,
            })
        }
        fn release(&self, _: &str) -> ContractResult<HardwareExecutionResult> {
            unreachable!()
        }
        fn executor_name(&self) -> &'static str {
            "ack_only"
        }
    }
    #[test]
    fn p0_ack_without_independent_snapshot_is_rejected_before_write() {
        let writes = Arc::new(AtomicUsize::new(0));
        let actuator = GuardedHardwareActuator::new(
            DeviceCapabilityMatrix {
                os: HostOs::Linux,
                vendor: HardwareVendor::Amd,
                device_id: "gpu:unit".into(),
                capabilities: HardwareCapabilitySet::new("gpu:unit")
                    .with_device_kind(
                        aicore_contracts::hardware::capability::HardwareDeviceKind::Gpu,
                    )
                    .with(HardwareCapabilityEntry::supported(
                        HardwareCapability::DeviceIdentity,
                    ))
                    .with(HardwareCapabilityEntry::supported(
                        HardwareCapability::WorkloadHint,
                    )),
                thermal_limit_celsius: None,
            },
            Some(Arc::new(AckOnly(writes.clone()))),
        );
        let result = actuator
            .apply_authorized(
                &HardwareExecutionRequest {
                    device_id: "gpu:unit".into(),
                    hint: HardwareExecutionHint::WorkloadPreference("compute".into()),
                },
                &ActuationPermit {
                    device_id: "gpu:unit".into(),
                    capability: HardwareCapability::WorkloadHint,
                    expires_at_unix_ms: 100,
                    model_only: false,
                },
                1,
            )
            .unwrap();
        assert!(
            !result.status.is_applied(),
            "command acknowledgement is not verified hardware state"
        );
        assert_eq!(
            writes.load(Ordering::SeqCst),
            0,
            "snapshot failure must precede mutation"
        );
    }
}

#[cfg(test)]
mod p0_transaction_tests {
    use super::*;
    use aicore_contracts::{
        errors::ContractError,
        hardware::{
            capability::{HardwareCapabilityEntry, HardwareCapabilitySet, HardwareDeviceKind},
            matrix::{HardwareVendor, HostOs},
        },
        traits::hardware_executor::HardwareStateSnapshot,
    };
    use std::sync::Mutex;
    #[derive(Clone, Copy)]
    enum Fault {
        None,
        Mismatch,
        PartialFailure,
        RestoreFailure,
        DelayedApply,
        Unsupported,
        ModelVerified,
    }
    struct Device {
        state: Mutex<Option<HardwareExecutionHint>>,
        fault: Fault,
    }
    impl IHardwareExecutor for Device {
        fn is_model_only(&self) -> bool {
            matches!(self.fault, Fault::ModelVerified)
        }
        fn supports_reversible_state(&self) -> bool {
            true
        }
        fn read_hardware_state(&self, id: &str) -> ContractResult<HardwareStateSnapshot> {
            Ok(HardwareStateSnapshot {
                device_id: id.into(),
                hint: self.state.lock().unwrap().clone(),
            })
        }
        fn restore_hardware_state(
            &self,
            state: &HardwareStateSnapshot,
        ) -> ContractResult<HardwareExecutionResult> {
            if matches!(self.fault, Fault::RestoreFailure) {
                return Err(ContractError::AdapterUnavailable("restore failed".into()));
            }
            *self.state.lock().unwrap() = state.hint.clone();
            Ok(HardwareExecutionResult {
                device_id: state.device_id.clone(),
                status: HardwareExecutionStatus::Applied,
                latency_ms: 0,
            })
        }
        fn apply(
            &self,
            request: &HardwareExecutionRequest,
        ) -> ContractResult<HardwareExecutionResult> {
            if matches!(self.fault, Fault::Unsupported) {
                return Ok(HardwareExecutionResult {
                    device_id: request.device_id.clone(),
                    status: HardwareExecutionStatus::NotSupported {
                        reason: "unsupported test operation".into(),
                    },
                    latency_ms: 0,
                });
            }
            if !matches!(self.fault, Fault::Mismatch) {
                *self.state.lock().unwrap() = Some(request.hint.clone());
            }
            if matches!(self.fault, Fault::DelayedApply) {
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            if matches!(self.fault, Fault::PartialFailure | Fault::RestoreFailure) {
                return Err(ContractError::AdapterUnavailable("partial apply".into()));
            }
            Ok(HardwareExecutionResult {
                device_id: request.device_id.clone(),
                status: if matches!(self.fault, Fault::ModelVerified) {
                    HardwareExecutionStatus::Verified
                } else {
                    HardwareExecutionStatus::Applied
                },
                latency_ms: 0,
            })
        }
        fn release(&self, _: &str) -> ContractResult<HardwareExecutionResult> {
            unreachable!()
        }
        fn executor_name(&self) -> &'static str {
            "test_device"
        }
    }
    fn matrix() -> DeviceCapabilityMatrix {
        DeviceCapabilityMatrix {
            os: HostOs::Linux,
            vendor: HardwareVendor::Amd,
            device_id: "unit:device".into(),
            capabilities: HardwareCapabilitySet::new("unit:device")
                .with_device_kind(HardwareDeviceKind::Gpu)
                .with(HardwareCapabilityEntry::supported(
                    HardwareCapability::DeviceIdentity,
                ))
                .with(HardwareCapabilityEntry::supported(
                    HardwareCapability::WorkloadHint,
                )),
            thermal_limit_celsius: None,
        }
    }
    fn request() -> HardwareExecutionRequest {
        HardwareExecutionRequest {
            device_id: "unit:device".into(),
            hint: HardwareExecutionHint::WorkloadPreference("compute".into()),
        }
    }
    fn permit() -> ActuationPermit {
        ActuationPermit {
            device_id: "unit:device".into(),
            capability: HardwareCapability::WorkloadHint,
            expires_at_unix_ms: u64::MAX,
            model_only: false,
        }
    }
    #[test]
    fn p0_independent_verification_is_distinct_from_command_acknowledgement() {
        let backend = Arc::new(Device {
            state: Mutex::new(None),
            fault: Fault::None,
        });
        let result = GuardedHardwareActuator::new(matrix(), Some(backend))
            .apply_authorized(&request(), &permit(), 1)
            .unwrap();
        assert!(
            !matches!(
                result.status,
                HardwareExecutionStatus::Applied | HardwareExecutionStatus::AlreadyInState
            ),
            "a verified observation must be distinguishable from legacy command acknowledgement"
        );
    }
    #[test]
    fn p0_model_backend_cannot_forge_native_verified_status() {
        let backend = Arc::new(Device {
            state: Mutex::new(None),
            fault: Fault::ModelVerified,
        });
        let mut model_permit = permit();
        model_permit.model_only = true;
        let result = GuardedHardwareActuator::new(matrix(), Some(backend))
            .apply_authorized(&request(), &model_permit, 1)
            .unwrap();
        assert!(matches!(result.status, HardwareExecutionStatus::Applied));
    }
    struct ConcurrentDevice {
        device: Device,
        active: std::sync::atomic::AtomicUsize,
        overlap: std::sync::atomic::AtomicBool,
    }
    impl IHardwareExecutor for ConcurrentDevice {
        fn supports_reversible_state(&self) -> bool {
            true
        }
        fn read_hardware_state(&self, id: &str) -> ContractResult<HardwareStateSnapshot> {
            self.device.read_hardware_state(id)
        }
        fn restore_hardware_state(
            &self,
            state: &HardwareStateSnapshot,
        ) -> ContractResult<HardwareExecutionResult> {
            self.device.restore_hardware_state(state)
        }
        fn apply(
            &self,
            request: &HardwareExecutionRequest,
        ) -> ContractResult<HardwareExecutionResult> {
            use std::sync::atomic::Ordering;
            if self.active.fetch_add(1, Ordering::SeqCst) > 0 {
                self.overlap.store(true, Ordering::SeqCst);
            }
            std::thread::sleep(std::time::Duration::from_millis(40));
            let result = self.device.apply(request);
            self.active.fetch_sub(1, Ordering::SeqCst);
            result
        }
        fn release(&self, _: &str) -> ContractResult<HardwareExecutionResult> {
            unreachable!()
        }
        fn executor_name(&self) -> &'static str {
            "concurrent test device"
        }
    }
    #[test]
    fn p0_distinct_guard_wrappers_serialize_shared_backend_transactions() {
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
        let backend = Arc::new(ConcurrentDevice {
            device: Device {
                state: Mutex::new(None),
                fault: Fault::None,
            },
            active: AtomicUsize::new(0),
            overlap: AtomicBool::new(false),
        });
        let barrier = Arc::new(std::sync::Barrier::new(3));
        let mut threads = Vec::new();
        for _ in 0..2 {
            let backend = backend.clone();
            let barrier = barrier.clone();
            threads.push(std::thread::spawn(move || {
                let guard = GuardedHardwareActuator::new(matrix(), Some(backend));
                barrier.wait();
                guard.apply_authorized(&request(), &permit(), 1).unwrap()
            }));
        }
        barrier.wait();
        for thread in threads {
            assert!(thread.join().unwrap().status.is_applied());
        }
        assert!(
            !backend.overlap.load(Ordering::SeqCst),
            "separate wrappers must not interleave on one backend"
        );
    }
    #[test]
    fn p0_permit_expiring_during_apply_restores_original_instead_of_verifying() {
        let backend = Arc::new(Device {
            state: Mutex::new(None),
            fault: Fault::DelayedApply,
        });
        let mut permit = permit();
        permit.expires_at_unix_ms = 20;
        let result = GuardedHardwareActuator::new(matrix(), Some(backend.clone()))
            .apply_authorized(&request(), &permit, 1)
            .unwrap();
        assert!(
            !result.status.is_applied(),
            "expired completed operation cannot become Verified"
        );
        assert_eq!(*backend.state.lock().unwrap(), None);
    }
    #[test]
    fn p0_unsupported_operation_preserves_truthful_status_after_original_is_verified() {
        let backend = Arc::new(Device {
            state: Mutex::new(None),
            fault: Fault::Unsupported,
        });
        let result = GuardedHardwareActuator::new(matrix(), Some(backend.clone()))
            .apply_authorized(&request(), &permit(), 1)
            .unwrap();
        assert!(matches!(
            result.status,
            HardwareExecutionStatus::NotSupported { .. }
        ));
        assert_eq!(*backend.state.lock().unwrap(), None);
    }
    #[test]
    fn p0_readback_mismatch_is_not_applied() {
        let backend = Arc::new(Device {
            state: Mutex::new(None),
            fault: Fault::Mismatch,
        });
        let result = GuardedHardwareActuator::new(matrix(), Some(backend.clone()))
            .apply_authorized(&request(), &permit(), 1)
            .unwrap();
        assert!(!result.status.is_applied());
        assert_eq!(*backend.state.lock().unwrap(), None);
    }
    #[test]
    fn p0_partial_apply_restores_absence_of_original_hint() {
        let backend = Arc::new(Device {
            state: Mutex::new(None),
            fault: Fault::PartialFailure,
        });
        let _ = GuardedHardwareActuator::new(matrix(), Some(backend.clone())).apply_authorized(
            &request(),
            &permit(),
            1,
        );
        assert_eq!(
            *backend.state.lock().unwrap(),
            None,
            "partial write must restore exact absent original"
        );
    }
    #[test]
    fn p0_restore_failure_remains_explicit_failure() {
        let backend = Arc::new(Device {
            state: Mutex::new(None),
            fault: Fault::RestoreFailure,
        });
        let result = GuardedHardwareActuator::new(matrix(), Some(backend))
            .apply_authorized(&request(), &permit(), 1)
            .unwrap();
        assert!(
            matches!(result.status, HardwareExecutionStatus::Failed { ref reason } if reason.contains("restor"))
        );
    }
    #[test]
    fn p0_valid_permit_with_independent_readback_applies() {
        let backend = Arc::new(Device {
            state: Mutex::new(None),
            fault: Fault::None,
        });
        let result = GuardedHardwareActuator::new(matrix(), Some(backend.clone()))
            .apply_authorized(&request(), &permit(), 1)
            .unwrap();
        assert!(result.status.is_applied());
        assert_eq!(
            *backend.state.lock().unwrap(),
            Some(HardwareExecutionHint::WorkloadPreference("compute".into()))
        );
    }
    #[test]
    fn p0_identity_matrix_mismatch_cannot_reach_device() {
        let backend = Arc::new(Device {
            state: Mutex::new(None),
            fault: Fault::None,
        });
        let mut matrix = matrix();
        matrix.capabilities.device_name = "other".into();
        let result = GuardedHardwareActuator::new(matrix, Some(backend.clone()))
            .apply_authorized(&request(), &permit(), 1)
            .unwrap();
        assert!(!result.status.is_applied());
        assert_eq!(*backend.state.lock().unwrap(), None);
    }
}
