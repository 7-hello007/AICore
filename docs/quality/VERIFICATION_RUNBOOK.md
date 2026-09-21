# Verification Runbook

```text
cargo fmt --all -- --check
cargo build --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
```

## Linux
- build release
- install systemd service
- start daemon and verify one complete tick
- verify Unix-domain IPC
- verify terminal and web dashboards use the same snapshot
- run L0 real-world scenarios
- run 1h stability smoke

## Windows
- build workspace
- install/start/stop service
- verify named pipe IPC
- verify process/power controls
- verify NVIDIA telemetry/control where `nvidia-smi` exists
- verify web dashboard

## AI
- load `models/policy_classifier_v1.json`
- verify local inference path
- verify fallback and timeout behavior
- verify no placeholder model is referenced

Static source inspection is not sufficient to close the release gate.
