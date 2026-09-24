# Destination work: isolated test boundary

The current authorization permits Model Mode, mocks, isolated filesystem/IPC
fixtures and **read-only** native telemetry/capability checks. The unfiltered
`cargo test --workspace --all-targets --locked` command also contains tests
that invoke platform process/affinity/power controllers or start a platform
daemon. Those can change host configuration, so the unfiltered command remains
**not run** without separate permission. The tests remain enabled.

`AICORE_EXECUTION_MODE=simulated-platform` selects the memory-only platform
adapter and the continuous `PlatformTickDriver`. Run
`bash scripts/verify/simulated_platform_smoke.sh` for an isolated daemon
smoke using the captured fixture described in
`docs/platform/PLATFORM_SIMULATOR.md`. This is a simulated Platform Mode pass,
not native hardware certification. Existing tests that instantiate native
controllers directly still need the filters below.

Run `bash scripts/verify/model_safe_workspace_tests.sh` for the workspace
suite. The script uses Cargo's `--skip` filters after reviewing the named
tests. The filters cover:

- direct process affinity, process controller, power controller and platform
  executor calls, including Linux GPU `release()` (which can write sysfs);
- daemon/release smoke tests that use default platform mode;
- live Linux CPU/GPU/NPU/temperature telemetry and L0 observations;
- end-to-end pipelines whose source is the live Linux adapter.

This conservative filter also excludes read-only live-hardware tests. Selected
adapter/capability targets can now be run separately after source review. A
filtered run must report its actual passed/failed/filtered counts. It does not
satisfy the unfiltered workspace gate or certify all target hosts.

The isolated `aicored --once` smoke uses `AICORE_EXECUTION_MODE=model`, its
own temporary paths for config/status/IPC/memory/plugins and `TMPDIR` for
legacy memory migration defaults. Its telemetry is read-only; its policy
executor and runtime binder are model implementations. The normal desktop
environment, services, packages, process priorities, affinity and power
profile are not changed.

Platform Mode binder/executor tests that may change process scheduling, power
or GPU state need a specific authorization and rollback plan first. Windows
native pipe behavior also needs a native toolchain and a Windows test host.

For a real Platform Mode write test, approval must identify the actual,
stable test PID/name and every device path or power target, requested value,
scope, captured before-state and rollback command. The current daemon derives
process targets from live telemetry when no explicit targets are supplied;
that dynamic target set is not an approvable frozen list. A read-only native
probe or a model test does not authorize a later setter call.
