# AICore V3.2 Engineering Completion Report

## 1. Package identity

- Workspace version: `0.3.2`
- Product stage: **V3.2 engineering-close implementation candidate**
- Product docs: global `docs/` only
- `destination/`: intentionally excluded from the product source package

## 2. Source-level problems addressed in this release

| Area | Closure in source |
|---|---|
| Workload fusion | source/observation pairing no longer depends on filtered indexes |
| Adaptive loop | TierHub-driven workload/task/adaptation providers are part of the main loop |
| Task queue | expired tasks are removed; multi-task dequeue and queue cleanup are supported |
| AI runtime | real local JSON model is the production baseline; one loaded runtime is shared by provider and routing |
| Memory | JSON cache ordering fixed; SQLite WAL retained; SQLite record timestamp column now has explicit millisecond semantics with legacy migration |
| Time model | outward event/task/execution timestamps use explicit millisecond fields; deadline fields retain their seconds contract where specified by the task API |
| IPC / dashboard | dashboard reads one daemon data plane; web SSE emits event ids and reconnect-safe current snapshots |
| Web dashboard | local-by-default binding, responsive modern visual design, live state, runtime/context/resource panels |
| Plugin process | stderr is drained, timeouts kill subprocesses, process-level host state is tracked |
| Windows service | service actions use installed configuration; upgrade/rollback no longer use a hard-coded binary path |
| Packaging | workspace version is the single packaging version source |
| Repository quality | flat tests, locked dependencies, license, JSON/TOML/script static checks |

## 3. Deliberate capability boundaries

GPU/NPU control is capability-gated. There is no universal cross-vendor control API that can honestly be reported as supported on every Windows/Linux machine. Unsupported paths must return `NotSupported` rather than fabricated success. Vendor-specific implementations remain behind the existing adapter/plugin contracts.

## 4. Verification status

Static repository checks completed in this environment:

- 109 test source files referenced by `tests/Cargo.toml`; all paths exist.
- `tests/` has no nested directories.
- All `Cargo.toml` files parsed successfully with Python TOML parser.
- All repository JSON files parsed successfully.
- `Cargo.lock` exists.
- `LICENSE` exists.
- `destination/` is absent from the product package.
- No model `.placeholder` artifact remains.
- Shell scripts pass `bash -n`.
- `scripts/audit_v32_static.sh` passes.

## 5. Release gate still requiring target-host evidence

The current execution environment does not have a Rust toolchain, and it cannot provide real Windows/Linux hardware/service evidence. Therefore the following are **not claimed as passed here**:

1. `cargo build --workspace --all-targets`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo nextest run --workspace` / `cargo test --workspace`
4. Windows Service install/start/stop/upgrade/rollback
5. Linux service/package installation
6. Real GPU/NPU telemetry and control on target hardware
7. 24h Windows/Linux nightly stability
8. Final installer/package smoke tests

Promotion to the next product stage must happen only after those gates produce CI/target-host evidence.
