#[cfg(target_os = "windows")]
#[path = "native.fixture.rs"]
mod native_fixture;
#[cfg(target_os = "windows")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use aicore_adapter_windows::{
        WindowsHardwareCapabilityProvider, WindowsHardwareTelemetryProvider,
    };
    use aicore_contracts::{
        ai::runtime::InferenceRequest,
        context::snapshot::ContextSnapshot,
        hardware::matrix::{DeviceCapabilityMatrix, HostOs},
        runtime::route::{RuntimeRoute, RuntimeRouteSource},
        traits::{
            hardware_telemetry::IHardwareTelemetryProvider, runtime_binder::BindingRequest,
            telemetry_provider::ITelemetryProvider,
        },
    };
    use std::sync::Arc;
    let npu_devices = WindowsHardwareTelemetryProvider::new().npu_devices();
    println!("READ ONLY NPU inventory={npu_devices:?}; identity discovery alone does not certify model execution");
    let project = std::env::current_dir()?;
    let ordinal = std::env::var("AICORE_ACCELERATOR_DEVICE_ID")?.parse::<u32>()?;
    let device = WindowsHardwareTelemetryProvider::new()
        .gpu_devices()
        .into_iter()
        .find(|d| d.id == format!("gpu:dxgi:{ordinal}"))
        .ok_or("configured hardware DXGI device absent")?;
    println!("NATIVE BEFORE target={:?} original=no AICore inference session proposed=isolated ONNX GPU inference rollback=unload model, release queue lease and exit child session; no power/clock setter",device);
    let caps = WindowsHardwareCapabilityProvider::new().gpu_capability(&device.id);
    let matrix = DeviceCapabilityMatrix::from_discovered(HostOs::Windows, &device, caps)
        .ok_or("matrix mismatch")?;
    let binder = aicore_runtime::queued_model_binder::configured_accelerator_binder(
        &project,
        matrix,
        Arc::new(aicore_runtime_router::DefaultRuntimeBinder::new()),
    )?;
    let result = binder.bind_and_execute(&BindingRequest {
        route: RuntimeRoute {
            route_id: "native-gpu-queue-probe".into(),
            source: RuntimeRouteSource::Gpu,
            applied: true,
            workload_type: aicore_contracts::context::workload::WorkloadType::Rendering,
            reason: "native GPU probe".into(),
            confidence: 1.0,
            decided_at_unix_secs: 0,
        },
        context: ContextSnapshot::default(),
        inference: Some(InferenceRequest {
            features: vec![0.1, 0.9, 0.0, 0.1],
            tensors: vec![],
        }),
    })?;
    if !result.executed
        || !result.inference.as_ref().is_some_and(|r| {
            r.labels
                .iter()
                .max_by(|a, b| a.score.total_cmp(&b.score))
                .is_some_and(|l| l.label == "boost_gpu")
        })
    {
        return Err(format!("GPU queue inference failed: {result:?}").into());
    }
    println!("PASS native GPU queue inference {result:?}");
    // Native telemetry and owned process control with the same split driver
    // used by the daemon. A fixed GPU route is a test stimulus, not production routing.
    use aicore_contracts::{
        errors::ContractResult, telemetry::snapshot::ComputerSnapshot,
        traits::runtime_router::IRuntimeRouter,
    };
    struct GpuProbeRoute;
    impl IRuntimeRouter for GpuProbeRoute {
        fn decide(
            &self,
            _: &ComputerSnapshot,
            _: &ContextSnapshot,
        ) -> ContractResult<RuntimeRoute> {
            Ok(RuntimeRoute {
                route_id: "native-gpu-fast-slow".into(),
                source: RuntimeRouteSource::Gpu,
                applied: true,
                workload_type: aicore_contracts::context::workload::WorkloadType::Rendering,
                reason: "explicit owned certification stimulus".into(),
                confidence: 1.0,
                decided_at_unix_secs: 0,
            })
        }
        fn name(&self) -> &'static str {
            "native_gpu_probe_route"
        }
    }
    use aicore_core::{
        context::{ContextEngine, RuleBasedClassifier},
        executor::{DefaultPolicyExecutor, PlatformPostConditionChecker, TransparentExecutor},
        policy::{DefaultPolicyValidator, RuleBasedPolicyEngine},
    };
    use std::time::{Duration, Instant};
    let process = Arc::new(native_fixture::OwnedProcess::new());
    let observed = aicore_adapter_windows::WindowsTelemetryProvider::new().collect()?;
    let complete_accelerator_metrics =
        observed.gpu.usage_percent.is_some() && observed.npu.usage_percent.is_some();
    println!("NATIVE READBACK gpu_utilization={:?} npu_utilization={:?}; missing metrics must not become zero feature values", observed.gpu.usage_percent, observed.npu.usage_percent);
    let executor = Arc::new(DefaultPolicyExecutor::new(Some(process.clone()), None));
    let transparent = Arc::new(
        TransparentExecutor::new(executor.clone()).with_checker(Arc::new(
            PlatformPostConditionChecker::new(Some(process.clone()), None),
        )),
    );
    let rule = Arc::new(RuleBasedPolicyEngine::new());
    let loop_ = Arc::new(
        aicore_runtime::AdaptiveLoop::new(
            Arc::new(aicore_adapter_windows::WindowsTelemetryProvider::new()),
            Arc::new(ContextEngine::new(Arc::new(RuleBasedClassifier::new()))),
            rule.clone(),
            rule,
            Arc::new(DefaultPolicyValidator::new()),
            executor,
            Default::default(),
        )
        .with_transparent_executor(transparent)
        .with_runtime_router(Arc::new(GpuProbeRoute))
        .with_runtime_binder(binder.clone()),
    );
    let driver = aicore_runtime::dual_loop::PlatformTickDriver::new(
        loop_,
        process.targets(),
        Duration::from_secs(3),
    )?;
    let sample_count = std::env::var("AICORE_NATIVE_DEADLINE_SAMPLES")
        .ok()
        .map(|v| v.parse::<usize>())
        .transpose()?
        .unwrap_or(200);
    if !(200..=10_000).contains(&sample_count) {
        return Err("deadline sample count must be 200..=10000".into());
    }
    let mut samples = Vec::with_capacity(sample_count);
    let mut gpu_executions = 0;
    let mut owned_control_executions = 0;
    for _ in 0..sample_count {
        let started = Instant::now();
        let tick = driver.tick();
        samples.push(started.elapsed().as_secs_f64() * 1000.0);
        if tick.runtime_binding_executed == Some(true) && tick.success {
            gpu_executions += 1;
        }
        if tick.success
            && tick.execution_status.is_some_and(|status| {
                matches!(status,
            aicore_contracts::execution::status::ExecutionOverallStatus::AllSucceeded |
            aicore_contracts::execution::status::ExecutionOverallStatus::PartiallySucceeded)
            })
        {
            owned_control_executions += 1;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    samples.sort_by(f64::total_cmp);
    let misses = samples.iter().filter(|v| **v > 20.0).count();
    println!("NATIVE SPLIT DEADLINE samples={sample_count} gpu_executions={gpu_executions} owned_control_executions={owned_control_executions} budget_ms=20 p99_ms={} max_ms={} misses={misses}; finite measurement not hard realtime guarantee",samples[(sample_count * 99).div_ceil(100) - 1],samples[sample_count - 1]);
    drop(driver);
    process.finish()?;
    // Bounded subprocess work already in progress may finish after shutdown;
    // it has no host setters. Wait for its isolated session cleanup before exit.
    std::thread::sleep(Duration::from_secs(6));
    if owned_control_executions == 0
        || (complete_accelerator_metrics && gpu_executions == 0)
        || (!complete_accelerator_metrics && gpu_executions != 0)
    {
        return Err("native fast loop or missing-utilization fail-closed assertion failed".into());
    }
    drop(binder);
    println!("RESTORATION PASS inference session/queue ownership released; GPU/power configuration unchanged (no setters invoked)");
    Ok(())
}
#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("NOT VERIFIED native Windows GPU on this OS");
}
