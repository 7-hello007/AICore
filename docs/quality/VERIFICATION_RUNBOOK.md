# Verification Runbook

## V4 verification modes

The full local suite contains nine host-control tests and was run unfiltered for V4 code closure. Build `aicore-daemon` and `plugin-echo` before process tests. Run Model Mode separately with `AICORE_EXECUTION_MODE=model` and a fresh temporary directory for config, status, IPC, plugins and memory (including `TMPDIR` for legacy migration defaults). Do not set the model variable globally over the full test suite.

The Linux/Windows installation instructions below describe separate target-host certification, not commands executed for V4 code closure. Package building and inspection were done without installation.

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
