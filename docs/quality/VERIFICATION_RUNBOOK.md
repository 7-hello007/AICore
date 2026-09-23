# Verification Runbook

## V4 model-only safety boundary

Do not run the unfiltered suite or platform installation steps below in a model-only verification session. Nine host-control tests are enumerated in `V4_DEFECT_CORRECTION_REPORT.md`; use its exact safe filters and report excluded tests separately. Build `aicore-daemon` and `plugin-echo` before process tests. Set `AICORE_EXECUTION_MODE=model` and use a fresh temporary directory for config, status, IPC, plugins and memory (including `TMPDIR` for legacy migration defaults).

The Linux/Windows installation instructions below describe separate target-host certification, NOT commands authorized by this V4 task.

```text
cargo fmt --all -- --check
cargo build --workspace --all-targets --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-targets --locked
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
