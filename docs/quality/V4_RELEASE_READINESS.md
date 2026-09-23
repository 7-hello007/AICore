# AICore V4 Release Readiness

## Current state

**Code engineering closure passed; deployment and broad platform certification remain open.**

## Required promotion evidence

- `cargo fmt --all -- --check`
- `cargo build --workspace --all-targets --locked` on Linux and Windows
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- complete `aicore-tests` manifest execution
- Linux and Windows service install/start/query/stop/uninstall smoke
- IPC malformed/oversized/duplicate/backpressure tests
- plugin trust, timeout, crash isolation and permission tests
- dashboard snapshot/SSE/browser smoke
- real GPU/NPU capability tests only where hardware/vendor API exists
- long-run daemon stability test

The V4 code engineering gate is evidenced in `V4_DEFECT_CORRECTION_REPORT.md`. Passing the additional target-host service and hardware gates is required for a certified production release; they do not reverse the code-level closure or rename the project V5.
