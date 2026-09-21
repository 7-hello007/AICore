# V3.2 Release Gate

V3.2 may be promoted to the next product stage only when every gate below is evidenced.

| Gate | Requirement | Evidence |
|---|---|---|
| Source | Workspace builds | `cargo build --workspace --all-targets` |
| Static | Clippy clean | `cargo clippy --workspace --all-targets -- -D warnings` |
| Tests | All tests pass | `cargo nextest run --workspace` or `cargo test --workspace` |
| Linux | Daemon/service smoke | Linux CI target |
| Windows | Service + IPC smoke | Windows CI target |
| AI | Local model loads | Runtime integration test |
| Dashboard | Terminal + Web agree | Dashboard integration/e2e test |
| Plugin | Isolation + timeout + signature | Plugin integration tests |
| L0 | Zero-integration scenarios pass | L0 real-world executable |
| Long run | Stable over the defined duration | Nightly evidence |

This repository was repaired statically in an environment without a Rust toolchain, so the gates above are not represented as passed until CI/target-host evidence is generated.
