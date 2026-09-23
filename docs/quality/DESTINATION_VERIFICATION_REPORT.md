# AICore destination implementation and verification report

Date: 2026-09-23. Source of scope: root `destination.md` and the current
request. This is **destination completion work in progress**, not a V4
defect-only closure or a claim of hardware certification. The item-by-item
requirement, current state, remaining gap and acceptance evidence are in
`DESTINATION_REQUIREMENTS.md`.

## Implemented in this work

- Platform execution now distinguishes absent verification from a passed
  postcondition. Successful process priority, throttle, affinity and power
  profile operations receive readback through the existing controller
  interfaces when supported; mismatches fail verification and enter the
  existing rollback path. `ControlTrace::is_success` follows its final status.
- Linux/Windows network byte deltas are converted to rates over elapsed time;
  Linux battery, AC and power profile readings use read-only power-supply
  data. Missing readings remain unavailable or best effort.
- AI policy calls have a bounded worker/deadline with rule fallback. This
  protects the policy call only; the entire destination fast/slow loop is not
  implemented.
- Linux IPC socket permissions and same-UID peer checks, a handshake/Ack
  exchange, bounded connections, idle timeout and duplicate request-ID
  rejection were added. A listener removes its socket on drop only when the
  path still identifies the socket it bound.
- The daemon retains its L1/L2 providers. IPC can declare/clear an L1 current
  task and submit/query/cancel an L2 task on the existing shared queue. L2
  cannot control a regular queue task through an arbitrary handle. A NoOp
  execution does not mark consumed tasks Completed.
- Plugin activation now uses an operator-pinned manifest and artifact digest,
  signature/trust check, bounded managed process and Ping/Pong health gate.
  Model Mode never launches external plugins. Only the existing generic
  `extension.execute` interface is bound; hardware/provider bindings remain.
- IPC registers an app ID to its OS peer identity and checks that identity on
  per-app actions. Linux uses peer UID/PID/start tick; Windows owner/System
  pipe ACL and client PID/creation time are implemented and cross-compiled,
  but not runtime-tested.
- Personalization loads history only when profile consent permits it, derives
  hourly workload patterns and uses those in short prediction. Consent and
  retention tests cover the in-memory backend. A before/after and control
  comparison framework accepts only observed metrics for verified feedback.
- Bounded intent-agent, consent-gated multi-agent coordinator, device model
  runtime catalog, vendor/OS capability matrix, guarded actuator contract
  and optional thermal/power policy guard were added with mocks. No actual
  LLM, GPU/NPU runtime or hardware backend is claimed.
- A separate immutable policy handoff/fast consumer bounds slow-provider
  effects in mock tests, including expiry and cancellation. The daemon still
  uses synchronous `AdaptiveLoop::tick`; full fast/slow integration remains.

These are source and isolated-behavior improvements. No destination row is
marked fully accepted while its integration, overhead or target-platform
evidence remains missing.

## Change inventory

| Files | Reason |
|---|---|
| `core/src/executor/{mod.rs,transparent.rs,post_condition.rs}`, `contracts/src/control/trace.rs` | Export platform readback checker; distinguish unverified state and final trace success |
| `adapters/linux/src/telemetry/{mod.rs,provider.rs,power.rs}`, `adapters/windows/src/telemetry/provider.rs` | Correct network rates; read Linux power-supply data |
| `ai/src/{lib.rs,bounded.rs,intent.rs,orchestration.rs}`, `apps/aicored/src/runtime.rs`, `ai-runtime/src/{lib.rs,devices.rs}` | Bound AI/intent calls, validate intent policy, coordinate mock agents and route registered model runtimes |
| `runtime/src/{adaptive_loop.rs,dual_loop.rs,lib.rs}`, `runtime/Cargo.toml` | Preserve NoOp/consent semantics and add immutable policy handoff component |
| `contracts/src/ipc/protocol.rs`, `ipc/src/{client.rs,server.rs,identity.rs,transport/linux.rs,transport/windows.rs}`, `apps/aicored/src/{cli.rs,ipc_handler.rs,main.rs,scheduler.rs}`, `apps/aicore-cli/src/main.rs`, `ipc/Cargo.toml` | IPC negotiation, OS peer identity, per-action authorization and L1/L2 application requests |
| `plugin/src/{activation.rs,host/process.rs,lib.rs}`, `apps/aicored/src/scheduler.rs` | Pinned fail-closed plugin activation and managed lifecycle; prevent duplicate crash records |
| `contracts/src/{effect.rs,ai/intent.rs,hardware/matrix.rs,memory/context.rs,traits/intent_agent.rs,traits/safe_actuator.rs}`, `core/src/{effect.rs,safe_actuator.rs,policy/thermal.rs}`, `memory/src/{personal.rs,lib.rs}` | Measurement, personal context, consent, intent and safe capability contracts |
| `tier-l1/src/task_source.rs`, `tier-l2/src/{lib.rs,native_provider.rs,register.rs}` | Retain current-task ownership and connect native handles to the existing queue |
| `tests/{control.trace.unit.test.rs,transparent.control.unit.test.rs,pipeline.taskPipeline.integration.test.rs,l2.standalone.unit.test.rs}` | Regression coverage for corrected execution and task semantics |
| `docs/contracts/IPC_PROTOCOL.md`, `docs/quality/ENGINEERING_STATUS.md` | Contract and status consistency |
| `docs/plugin/{PLUGIN_LIFECYCLE.md,PLUGIN_SECURITY.md}`, `docs/runtime/RUNTIME_ARCHITECTURE.md`, `docs/memory/MEMORY_ARCHITECTURE.md` | Distinguish destination design from code currently wired and tested |
| `docs/quality/{DESTINATION_REQUIREMENTS.md,DESTINATION_TEST_SAFETY.md,DESTINATION_VERIFICATION_REPORT.md}`, `scripts/verify/model_safe_workspace_tests.sh` | Requirement matrix, safe validation and current evidence |

The inventory includes both modified and new files; there are no deleted
source files. The seven protected residual files were neither edited nor
renamed.

Initial destination batch paths (additional paths are in the table above and
the final working-tree diff; this is not a complete changed-path list):

```text
adapters/linux/src/telemetry/mod.rs
adapters/linux/src/telemetry/provider.rs
adapters/linux/src/telemetry/power.rs
adapters/windows/src/telemetry/provider.rs
ai/src/lib.rs
ai/src/bounded.rs
apps/aicore-cli/src/main.rs
apps/aicored/src/ipc_handler.rs
apps/aicored/src/runtime.rs
apps/aicored/src/scheduler.rs
contracts/src/control/trace.rs
contracts/src/ipc/protocol.rs
core/src/executor/mod.rs
core/src/executor/transparent.rs
core/src/executor/post_condition.rs
docs/contracts/IPC_PROTOCOL.md
docs/quality/ENGINEERING_STATUS.md
docs/quality/DESTINATION_REQUIREMENTS.md
docs/quality/DESTINATION_TEST_SAFETY.md
docs/quality/DESTINATION_VERIFICATION_REPORT.md
ipc/src/client.rs
ipc/src/server.rs
ipc/src/transport/linux.rs
runtime/src/adaptive_loop.rs
scripts/verify/model_safe_workspace_tests.sh
tests/control.trace.unit.test.rs
tests/l2.standalone.unit.test.rs
tests/pipeline.taskPipeline.integration.test.rs
tests/transparent.control.unit.test.rs
tier-l1/src/task_source.rs
tier-l2/src/lib.rs
tier-l2/src/native_provider.rs
tier-l2/src/register.rs
```

Additional new files include `plugin/src/activation.rs`, `ipc/src/identity.rs`,
`runtime/src/dual_loop.rs`, `memory/src/personal.rs`, `ai/src/{intent.rs,orchestration.rs}`,
`ai-runtime/src/devices.rs`, `contracts/src/{effect.rs,ai/intent.rs,hardware/matrix.rs,traits/intent_agent.rs,traits/safe_actuator.rs}`
and `core/src/{effect.rs,safe_actuator.rs,policy/thermal.rs}`. Deleted source files: **none**.

## Validation evidence

| Check | Result | Scope |
|---|---|---|
| `cargo metadata --no-deps --format-version 1` | PASS, exit 0 | Workspace inventory |
| `cargo check --workspace --all-targets --locked` | PASS, exit 0 | WSL/Linux workspace |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | PASS, exit 0 | WSL/Linux workspace |
| `cargo fmt --all -- --check` | PASS, exit 0 | Workspace formatting |
| `bash scripts/verify/model_safe_workspace_tests.sh` | PASS, exit 0: 137 test targets; 666 passed, 0 failed, 0 ignored, 43 filtered | Safe subset of workspace tests; see `DESTINATION_TEST_SAFETY.md` |
| `cargo test --workspace --all-targets --locked` without filters | SKIPPED | Contains prohibited live-control/live-host tests |
| `TMPDIR=/tmp/aicore-destination2-smoke AICORE_EXECUTION_MODE=model cargo run -p aicore-daemon --bin aicored --locked -- --once --config /tmp/aicore-destination2-smoke/config.json --state-path /tmp/aicore-destination2-smoke/status.json --ipc-path /tmp/aicore-destination2-smoke/aicored.sock --memory-path /tmp/aicore-destination2-smoke/memory.db --plugins-dir /tmp/aicore-destination2-smoke/plugins --log-format json` | PASS, exit 0: one simulated tick, six model outcomes, 1 loaded JSON model | Route was native and binding reported no backend action; this is not hardware success or full AI-route evidence |
| `modelNORealPC.execution.unit.test.rs` in the safe workspace run | PASS | Mock/model AI CPU route, binder, task, memory and event chain |
| `cargo check -p aicore-ipc -p aicore-adapter-windows -p aicore-tier-l2 -p aicore-core --all-targets --locked --target x86_64-pc-windows-msvc` and `cargo check -p aicore-plugin -p aicore-ai -p aicore-ai-runtime --all-targets --locked --target x86_64-pc-windows-msvc` | PASS, exit 0 | Targeted cross-compile only; no Windows runtime |
| `cargo check -p aicore-daemon --all-targets --locked --target x86_64-pc-windows-msvc` | FAIL, exit 1: bundled SQLite build dependency cannot find MSVC `lib.exe` in WSL | Toolchain/environment limitation; daemon Windows target not code-validated here |
| `bash scripts/audit_v4_static.sh` | PASS, exit 0: 110 Rust test files checked | Static invariants |
| `bash scripts/audit_v32_static.sh` | PASS, exit 0 | Historical static structure checks |
| `git ls-files -z "*.sh" \| xargs -0 -n1 bash -n` plus new safety script | PASS, exit 0 | Shell syntax |

This continuation recorded file inventory in
`/tmp/aicore-destination2-files-before.txt` and compared it with
`/tmp/aicore-destination2-files-after.txt`. No pre-existing non-`target/`
file was removed. The 7 explicitly protected residual files have identical
before/after SHA-256 hashes. Cargo replaced generated files under `target/`;
those transient build artifacts are excluded from source-file integrity.

The earlier `V4_DEFECT_CORRECTION_REPORT.md` records a different validation
session; its unfiltered 666-test result is not presented as current evidence.

## Remaining destination work and specific blockers

- **MVP effect/feedback:** data structures and conservative comparison logic
  exist, but live paired latency/throughput/energy collection and feedback to
  the daemon policy engine do not. Model ticks cannot establish benefit.
- **Plugin lifecycle:** pinned generic extension activation has mock coverage,
  but typed telemetry/controller capability binding and full crash/restart
  policy are absent. Windows process permissions need native validation.
- **Application identity and IPC:** same-UID Linux peers and process-lifetime
  identity binding exist. App signing/SID/token attestation, stale registration
  cleanup, native pipe ACL behavior and compatible upgrades still need work.
- **Fast/slow control:** the new handoff tests prove a bounded *component*,
  not the daemon control deadline. Telemetry, history, prediction and runtime
  binding remain synchronous in `AdaptiveLoop::tick`; late binding could still
  delay control. Integrating the component with SafeMode and the scheduler is
  the central code blocker.
- **V2:** the abstract intent agent and device catalog have no actual local
  LLM provider/model, daemon intent entry point or GPU/NPU model runtime.
  Hourly pattern prediction has no prospective accuracy/benefit dataset.
- **V3:** matrix and guarded-actuator abstractions are not populated from
  supported devices or wired to the platform binder. The optional thermal
  guard is not enabled by the daemon. Advanced scheduling and measured power
  optimization remain absent. Kernel integration stays conditional.
- **Release/cross-platform:** Windows native daemon/service behavior, Linux
  package install, multi-vendor hardware, long-run overhead and thermal safety
  remain unverified. The targeted Windows compile is not whole-workspace or
  runtime certification.

## Final real-machine stage, not executed here

1. On controlled Windows and Linux hosts, run all 43 filtered tests and the
   complete unfiltered workspace suite after checking each test's side effect.
2. Compare telemetry to independent readings across supported CPU, GPU, NPU,
   storage, network, battery and thermal devices; record unavailable cases.
3. Exercise process/power control with before/after readback, performance and
   energy effect, permission denial, rollback and SafeMode under a recovery
   procedure.
4. Verify Windows named-pipe ACL/identity, service lifecycle and package
   installation in disposable hosts; verify Linux package/service lifecycle
   similarly. No installer or service action was run in this task.
5. Measure sustained fast-loop deadlines, slow-loop fallback, idle overhead,
   prediction calibration and user-workload benefit against the acceptance
   model before marking destination completion.

**Decision:** destination completion is **NO / NOT VERIFIED**. The safe
workspace code gate passes for the current changes, but the listed product
requirements and controlled-machine acceptance are still open.
