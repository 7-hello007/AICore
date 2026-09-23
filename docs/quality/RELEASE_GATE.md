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

Current evidence is in `V4_DEFECT_CORRECTION_REPORT.md`. The table retains broader platform-release requirements; real service/hardware rows are prohibited in the present model-only task and are NOT RUN. The unfiltered test gate remains open because nine tests perform real host control. No test is removed or marked ignored to bypass this gate.
