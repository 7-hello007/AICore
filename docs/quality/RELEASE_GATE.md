# V4 Release Gate

V4 is defect repair and engineering closure. Promotion requires evidence for the current scope; model verification must never be described as hardware certification.

| Gate | Requirement | Evidence |
|---|---|---|
| Source | Workspace builds | `cargo build --workspace --all-targets --locked` |
| Static | Clippy clean | `cargo clippy --workspace --all-targets --locked -- -D warnings` |
| Tests | All tests pass | `cargo nextest run --workspace` or `cargo test --workspace` |
| Linux | Daemon/service smoke | Linux CI target |
| Windows | Service + IPC smoke | Windows CI target |
| AI | Local model loads | Runtime integration test |
| Dashboard | Terminal + Web agree | Dashboard integration/e2e test |
| Plugin | Isolation + timeout + signature | Plugin integration tests |
| L0 | Zero-integration scenarios pass | L0 real-world executable |
| Long run | Stable over the defined duration | Nightly evidence |

Current evidence is in `V4_DEFECT_CORRECTION_REPORT.md`. The unfiltered Linux test gate is closed (666/666 nextest; no filters); isolated Model Mode is separately verified. The table also contains broader deployment and platform certification gates: Windows runtime, service installation and package installation were not run in this task and are not evidence for code engineering closure.
