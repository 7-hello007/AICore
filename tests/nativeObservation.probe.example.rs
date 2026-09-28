#[cfg(target_os = "windows")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use aicore_contracts::traits::telemetry_provider::ITelemetryProvider;
    use std::collections::BTreeSet;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    if std::env::var("AICORE_OBSERVATION_CONSENT").as_deref() != Ok("yes") {
        return Err("explicit AICORE_OBSERVATION_CONSENT=yes required".into());
    }
    let root = std::env::current_dir()?;
    let output_dir = root.join("env").canonicalize()?;
    let count = std::env::var("AICORE_OBSERVATION_SAMPLES")
        .ok()
        .map(|v| v.parse::<usize>())
        .transpose()?
        .unwrap_or(30);
    if !(10..=120).contains(&count) {
        return Err("sample count must be 10..=120".into());
    }
    let provider = aicore_adapter_windows::WindowsTelemetryProvider::new();
    let mut samples = Vec::with_capacity(count);
    let mut apps = BTreeSet::new();
    eprintln!("READ ONLY target=Windows telemetry original=no AICore sample proposed={count} one-second samples, no window titles or host setters; rollback=stop sampling, no system setting to restore");
    for index in 0..count {
        let snapshot = provider.collect()?;
        let foreground = snapshot.applications.iter().find(|app| app.is_foreground);
        if let Some(app) = foreground {
            apps.insert(app.name.clone());
        }
        samples.push(serde_json::json!({
            "timestamp_unix_ms": snapshot.collected_at_unix_ms,
            "foreground_process_name": foreground.map(|a| a.name.as_str()),
            "foreground_pid": foreground.map(|a| a.pid),
            "cpu_usage_percent": snapshot.cpu.usage_percent,
            "gpu_usage_percent": snapshot.gpu.usage_percent,
            "npu_usage_percent": snapshot.npu.usage_percent,
            "memory_usage_percent": snapshot.memory.usage_percent,
            "window_title": null,
            "source": "observed_windows_adapter"
        }));
        if index + 1 < count {
            std::thread::sleep(Duration::from_secs(1));
        }
    }
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let path = output_dir.join(format!("readonly-observation-{stamp}.json"));
    std::fs::write(&path, serde_json::to_vec_pretty(&samples)?)?;
    println!("PASS observed Windows samples={} distinct_foreground_names={} missing_gpu_utilization={} missing_npu_utilization={} output={} (no task registration or personal-memory acceptance claimed)",
        samples.len(), apps.len(), samples.iter().filter(|s| s["gpu_usage_percent"].is_null()).count(),
        samples.iter().filter(|s| s["npu_usage_percent"].is_null()).count(),path.display());
    println!("RESTORATION PASS telemetry was read-only; no process/power/GPU/NPU setter invoked");
    Ok(())
}
#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("NOT VERIFIED native Windows observation on this OS");
}
