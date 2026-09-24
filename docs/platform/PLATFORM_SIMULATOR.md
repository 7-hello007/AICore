# Platform Simulator

`AICORE_EXECUTION_MODE=simulated-platform` runs the daemon's **Platform Mode**
pipeline with `adapters/simulated-platform`. Continuous operation uses the
existing `PlatformTickDriver`; `--once` uses a single synchronous tick. The
adapter implements the shared telemetry, capability, process, power, GPU,
NPU, hardware telemetry, hardware execution and guarded actuator contracts.
Its mutable state is process-local memory. No simulator method opens a host
process, device or power control handle, and plugin subprocess activation is
disabled in this mode. The daemon itself still writes status, IPC and
optional SQLite memory files; use isolated paths for test runs.

The bundled fixture is
`adapters/simulated-platform/fixtures/linux_wsl_readonly.json` (SHA-256
`8166fe921a16b46bc035227a8f8a9603226d3f242bc04cea34556f9529e7dbdb`).
It was captured on 2026-09-24 by running the read-only Linux telemetry,
capability, process *getter*, power *getter* and hardware telemetry adapters
through `cargo run -p aicore-adapter-simulated-platform --example capture_linux`.
The capture program redacts process names, application/window data, mount
paths and network interface names while retaining measured numeric values.
Its `snapshot.collected_at_unix_ms` records the acquisition time. No values
were inserted to create unavailable GPU, NPU or power controls. The simulator
rejects malformed capabilities, synthetic process IDs absent from the
recorded snapshot and hardware matrices without corresponding sampled devices.

The fixture contains captured PIDs only as **in-memory keys**. The daemon
selects the first captured PID as a simulator foreground target and the next
as a simulator background target to exercise the existing executor. This is
a test scenario, not a claim that either process was the real foreground app.
An absent PID/device/profile returns a capability error or `NotSupported`.
Readback and rollback observe the same in-memory state. Fault hooks can make
one process write fail or one post-write readback stale.

Run `bash scripts/verify/simulated_platform_smoke.sh` to build and run an
isolated one-tick and three-second continuous daemon session. The script
creates a fresh temporary directory for migration defaults, config, status,
IPC, plugins and SQLite; it leaves artifacts there for review. Unit tests
exercise readback, unsupported devices, fixture consistency and affinity
restoration. Daemon tests exercise slow-provider expiry/deadline, fresh plan
apply, verification failure/rollback and repeated failure to SafeMode.

**Scope of evidence:** the simulator proves contract wiring and state
transitions for this recorded Linux/WSL sample. It does not certify native
Windows pipes, actual priority/affinity/power or GPU/NPU writes, sensor
accuracy, performance benefit, or a hard deadline when native binding itself
blocks. Those require separate target-host evidence and explicit approval
before any configuration-changing operation.
