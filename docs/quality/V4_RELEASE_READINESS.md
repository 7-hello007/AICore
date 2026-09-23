# AICore V4 Release Readiness

## Current state

**Candidate, not certified release.**

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

Passing these gates is the condition for marking V4 release-ready and beginning the next product-development stage.
