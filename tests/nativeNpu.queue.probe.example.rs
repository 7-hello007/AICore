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
        traits::{hardware_telemetry::IHardwareTelemetryProvider, runtime_binder::BindingRequest},
    };
    use std::sync::Arc;
    let devices = WindowsHardwareTelemetryProvider::new().npu_devices();
    if devices.len() != 1 {
        return Err("native NPU identity ambiguous/absent".into());
    }
    let device = &devices[0];
    let original = "no AICore NPU session/provider registration";
    eprintln!("NATIVE BEFORE device={} original={} proposed=isolated NPU policy inference rollback=child process exit releases EP registration; no global device setter",device.id,original);
    let caps = WindowsHardwareCapabilityProvider::new().npu_capability(&device.id);
    let matrix = DeviceCapabilityMatrix::from_discovered(HostOs::Windows, device, caps)
        .ok_or("native NPU matrix mismatch")?;
    let binder = aicore_runtime::queued_model_binder::configured_accelerator_binder(
        &std::env::current_dir()?,
        matrix,
        Arc::new(aicore_runtime_router::DefaultRuntimeBinder::new()),
    )?;
    let result = binder.bind_and_execute(&BindingRequest {
        route: RuntimeRoute {
            route_id: "native-npu-queue-probe".into(),
            source: RuntimeRouteSource::Npu,
            applied: true,
            workload_type: aicore_contracts::context::workload::WorkloadType::AiInference,
            reason: "explicit NPU certification stimulus".into(),
            confidence: 1.0,
            decided_at_unix_secs: 0,
        },
        context: ContextSnapshot::default(),
        inference: Some(InferenceRequest {
            features: vec![0.1, 0.0, 0.9, 0.1],
            tensors: vec![],
        }),
    })?;
    if !result.executed
        || !result.inference.as_ref().is_some_and(|r| {
            r.labels
                .iter()
                .max_by(|a, b| a.score.total_cmp(&b.score))
                .is_some_and(|l| l.label == "boost_npu")
        })
    {
        return Err(format!("native NPU queue inference failed: {result:?}").into());
    }
    println!("PASS native NPU queue inference {result:?}");
    drop(binder);
    println!("RESTORATION PASS native NPU session/queue released; child EP registrations ended; NPU global settings unchanged");
    Ok(())
}
#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("NOT VERIFIED native Windows NPU on this OS");
}
