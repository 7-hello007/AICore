# Destination work: isolated test boundary

## Current desktop authorization (2026-09-27)

The user permits native operations and system changes for this continuation,
but prohibits installer packages and confines downloads to project `env/`.
The five Windows filters below remain for the **isolated suite** because those
tests do not identify a fresh owned process or guarantee exact restoration.
This is a deliberately filtered pass, not an unfiltered workspace pass.
`owned_process_probe` is a separate, explicit native example. It changes only
its freshly spawned Normal-priority child to AboveNormal, verifies readback,
restores Normal, verifies restoration and terminates/reaps that child. It
refuses a non-Normal inherited class to avoid a lossy enum restoration. No
affinity, power, GPU/NPU or service changes are included. Results are recorded
separately in the current destination verification report.

Pins now also accept `allowed_principals`, resolved from peer token SID on
Windows or UID on Linux. A configured nonempty list is additive to artifact
and per-action authorization; missing principal fails closed. Cross-user ACL
certification is still separate. Intent backend configuration is documented
in [INTENT_BACKENDS](../ai/INTENT_BACKENDS.md). Historical sections below retain
the authority and results of earlier runs; they are not current instructions.

## Windows desktop code continuation (2026-09-24)

Use the Windows desktop checkout only. The reviewed Windows-safe workspace run
uses `cargo test --workspace --all-targets --locked --offline --` with these
five `--skip` filters: `controller_handles_bad_pid_gracefully`,
`executor_runs_and_reports_outcomes`, `full_loop_runs_and_records_metrics`,
`history_disabled_by_default_means_no_records`, and
`history_enabled_records_tick`. The first invokes a process setter even
though its PID is intended to be invalid; the latter four construct real
process/power controllers and may apply a policy. These tests are retained.
The filtered run is not an unfiltered workspace pass. The run completed with
141 targets, 711 passed, 0 failed, 0 ignored and 5 filtered.

`AICORE_APP_POLICY_PATH` may point to an **absolute path** containing a JSON
array of operator-approved applications. Example schema:

```json
[{"application_id":"aicore.cli","artifact_sha256":"<64 hex characters>","allowed_actions":["register","propose_intent","unregister"]}]
```

The daemon reads this file at startup. When configured, every application
action requires the authenticated IPC peer's executable SHA-256 and a matching
per-action pin. Missing/unreadable executable, absent pin or revoked action is
denied. The digest is a read-only file hash, not a Windows code-signing/SID or
cross-user ACL certificate. Without this optional policy, the prior
process-bound IPC behavior remains.

`AICORE_INTENT_BACKEND=ollama` (default) exposes a semantic `ProposeIntent`
IPC request; `disabled` returns `NotSupported`. `AICORE_OLLAMA_MODEL` defaults
to `llama3.2:3b`, with loopback host, port and timeout configurable through
`AICORE_OLLAMA_HOST`, `AICORE_OLLAMA_PORT` and
`AICORE_OLLAMA_TIMEOUT_MS`. The provider connects only to loopback and does
not start or install Ollama. `aicore intent --text "..." --ipc-path <pipe>`
registers the calling application, requests a semantic proposal, then
unregisters. An operator pin must grant those three actions. The response is
not an executable policy; any policy conversion still passes the existing
policy engine and validator.

`scripts/verify/simulated_platform_ollama_intent.ps1` runs only a project-built
simulated daemon, local IPC and a pre-existing Ollama service. It uses scratch
files under `env/`, stops only its test daemon and never invokes a platform
setter. Its result is a local intent integration result, not a Windows
hardware or ACL certification.

The current authorization permits Model Mode, mocks, isolated filesystem/IPC
fixtures and **read-only** native telemetry/capability checks. The unfiltered
`cargo test --workspace --all-targets --locked` command also contains tests
that invoke platform process/affinity/power controllers or start a platform
daemon. Those can change host configuration, so the unfiltered command remains
**not run** without separate permission. The tests remain enabled.

`AICORE_EXECUTION_MODE=simulated-platform` selects the memory-only platform
adapter and the continuous `PlatformTickDriver`. Run
`bash scripts/verify/simulated_platform_smoke.sh` for an isolated daemon
smoke on Linux, or `scripts/verify/simulated_platform_windows_smoke.ps1`
on Windows, using the host-specific captured fixture described in
`docs/platform/PLATFORM_SIMULATOR.md`. This is a simulated Platform Mode pass,
not native hardware certification. Existing tests that instantiate native
controllers directly still need the filters below.

Run `bash scripts/verify/model_safe_workspace_tests.sh` for the workspace
suite. The script uses Cargo's `--skip` filters after reviewing the named
tests. The filters cover:

- direct process affinity, process controller, power controller and platform
  executor calls, including Linux GPU `release()` (which can write sysfs);
- daemon/release smoke tests that use default platform mode; `system_smoke`
  now uses isolated telemetry and no controllers, so its three tests run;
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
native same-user named-pipe handshake, Ack and peer identity were exercised
on a Windows desktop; cross-user ACL denial and service behavior remain
unverified.

For a real Platform Mode write test, approval must identify the actual,
stable test PID/name and every device path or power target, requested value,
scope, captured before-state and rollback command. The current daemon derives
process targets from live telemetry when no explicit targets are supplied;
that dynamic target set is not an approvable frozen list. A read-only native
probe or a model test does not authorize a later setter call.
