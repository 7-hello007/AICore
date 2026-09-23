# Destination work: isolated test boundary

This task uses Model Mode, mocks and isolated filesystem/IPC fixtures. The
unfiltered `cargo test --workspace --all-targets --locked` command contains
tests that invoke platform process/affinity/power controllers, collect live
hardware telemetry, or run a daemon against the host. It is therefore **not
run** under this task's validation boundary. The tests remain enabled and
unchanged for the final controlled real-machine stage.

Run `bash scripts/verify/model_safe_workspace_tests.sh` for the workspace
suite. The script uses Cargo's `--skip` filters after reviewing the named
tests. The filters cover:

- direct process affinity, process controller, power controller and platform
  executor calls;
- daemon/release smoke tests that use default platform mode;
- live Linux CPU/GPU/NPU/temperature telemetry and L0 observations;
- end-to-end pipelines whose source is the live Linux adapter.

This filtering also excludes read-only live-hardware tests because the task
permits only Model Mode, mocks and isolated tests. A filtered run must report
its actual passed/failed/filtered counts. It does not satisfy the unfiltered
workspace test gate or certify any target host.

The isolated `aicored --once` smoke uses `AICORE_EXECUTION_MODE=model`, its
own temporary paths for config/status/IPC/memory/plugins and `TMPDIR` for
legacy memory migration defaults. Its telemetry is read-only; its policy
executor and runtime binder are model implementations. The normal desktop
environment, services, packages, process priorities, affinity and power
profile are not changed.

The final real-machine stage should review the skipped test names and run
them on controlled Windows/Linux hosts with an explicit rollback procedure.
That stage is outside this task.
