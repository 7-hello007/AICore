# AICore destination implementation and verification report

## Continuation: 2026-09-27 (desktop backends, authorization and scheduling)

This continuation modifies only `C:\Users\12282\Desktop\AICore_V4_Windows`.
The WSL checkout is not synchronized or modified. This is destination code
development, not a claim that these additions are V4 defect fixes or that the
destination product/hardware acceptance gate is complete.

### Delivered code and evidence

| Requirement | Implemented behavior | Verified scope |
|---|---|---|
| ONNX / local JSON intent | Configurable runtime-backed vocabulary/typed-label adapter; checked-in weighted JSON and matching real MatMul/Softmax ONNX; isolated CPU-only Python ONNX Runtime factory with schema validation, bounded streams and subprocess timeout | Actual JSON and ONNX CPU inference; simulated-daemon intent IPC |
| Advanced GPU/NPU scheduling | Bounded admission, priority/earliest deadline/FIFO placement, registered matrix and backend slots, thermal guard, cancellation/expiry and immutable single-dispatch lease through safe actuator | Mock admission/thermal/capacity/dispatch tests; no native device scheduling claim |
| Agent orchestration | Concurrent bounded provider proposals, privacy filtering, confidence votes, disagreement rejection and minimum expiry; daemon ensemble selects local JSON + Ollama | Three slow mock agents share one wall deadline; actual ensemble IPC asserts both providers contributed |
| Effect feedback | Shared FeedbackPolicyEngine wraps daemon AI generation and rule fallback; trusted last-applied-policy ingestion rejects model execution; verified regression bypasses matching primary policy for up to one hour | Deterministic regression/model rejection tests and separate observed child-workload probe; no general daemon workload collector or product benefit claim |
| Cross-major IPC | Explicit 1.0/2.0 negotiation; v2 requires handshake session, uses typed payload parity and selected-version Ack/response; unknown majors and standalone v2 reject | Native same-user Windows named-pipe v2 roundtrip; contracts and Linux-target code check |
| Artifact/SID policy | Transport-resolved read-only executable SHA-256 and Windows token SID/Linux UID; optional principal list adds to per-action pin checks | Native same-user artifact+SID acceptance and mismatched SID denial; no cross-user ACL/signature attestation |
| Personal pre-task actuation | Consented retained application/hour pattern creates delayed preparation task; apply rechecks current owner/peer/pins/consent/expiry; process-only targets skip global power and runtime binding | Daemon+Model consent-revocation/apply test; delayed queue, cancellation, expiry and guard cleanup tests; empty process-only targets are NoOp |
| Documentation | Current requirement inventory, IPC specification, intent configuration, scheduling/feedback and safe test boundary | Synced to source and this report |

The JSON/ONNX asset is a hand-authored classifier, not a trained generative LLM
or calibrated accuracy model. ONNX subprocess sessions are not persistent.
A rule fallback may still produce a similar resource shape; choosing it does
not guarantee reversal of an observed regression. Pipeline latency and model
effects are not evidence of application performance improvement.

### Project-level verification

All commands use the existing Rust **1.75.0** toolchain, project-local
`env/cargo-home`, `--locked` and `--offline`. No MSRV upgrade.

| Command/check | Actual result |
|---|---|
| `cargo check --workspace --all-targets --locked --offline` | **PASS**, exit 0; `env/validation/desktop-check-final.log` |
| `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings` | **PASS**, exit 0; `env/validation/desktop-clippy-final.log` |
| `cargo fmt --all -- --check` | **PASS**, exit 0 |
| Safe workspace test command below | **PASS**, exit 0; 143 executed targets, **719 passed, 0 failed, 0 ignored, 5 filtered**; `env/validation/desktop-safe-tests-final.log` |
| `aicored --once` with `AICORE_EXECUTION_MODE=model` and isolated paths | **PASS**, exit 0; 1 model tick/1 model success/0 failures; `env/model-final-1790515502945` |
| `scripts/verify/simulated_platform_windows_smoke.ps1` | **PASS**, exit 0; 10 ticks, 5 in-memory successes, 0 failures, pipe status/ticks; `env/windows-smoke-1790515504398` |
| `simulated_platform_ollama_intent.ps1 -Backend json/onnx/ollama/ensemble` (each separately) | **PASS** for all four; compilation confidence 0.9522384 / 0.9522384 / 1 / 0.97611916. Ensemble assertion requires two contributors. Actual Ollama model: `llama3.2:3b` |
| `simulated_platform_ollama_intent.ps1 -Backend json -RejectSid` | **PASS**: mismatched SID rejected over native same-user pipe |
| PowerShell parser for 9 repository scripts; Python AST for both new model scripts | **PASS**, exit 0; no script installation/control execution |
| Applicable structural audit | **PASS**: Cargo/docs/license exist, no destination directory/model placeholders/nested tests/stale versioned doc links |
| `cargo check --workspace --all-targets --target x86_64-unknown-linux-gnu --locked --offline` | **PASS** on final run, exit 0 with project-local portable Zig C cross compiler; `env/validation/linux-cross-check-final.log`. Initial attempt **FAIL**ed with missing compiler (exit 101), retained in `env/validation/linux-cross-check.log` |
| `cargo clippy --workspace --all-targets --all-features --target x86_64-unknown-linux-gnu --locked --offline -- -D warnings` | **PASS**, exit 0 with the same project-local compiler; `env/validation/linux-cross-clippy-final.log`. Compile/static check only, no native Linux execution |
| Linux-target check for contracts/core/api/ai/ai-runtime/ipc/task-runtime, all targets | **PASS**, exit 0; `env/validation/linux-cross-code-check.log`. This is not the entire Linux workspace or a Linux execution result |

Exact isolated suite command:

```powershell
$env:AICORE_SMOKE_SECONDS='1'
cargo test --workspace --all-targets --locked --offline -- --skip controller_handles_bad_pid_gracefully --skip executor_runs_and_reports_outcomes --skip full_loop_runs_and_records_metrics --skip history_disabled_by_default_means_no_records --skip history_enabled_records_tick
```

These five tests remain enabled in source; this run is **filtered**, not an
unfiltered test pass. They can use non-owned process targets or system power
setters without exact restoration. Installer/service tests present in this
suite are static or mocks, not package/SCM installation. Linux-only cases have
no native execution coverage on this Windows host. Shell scripts were reviewed;
no WSL execution/synchronization was used to run them.

An initial ONNX daemon smoke **FAIL**ed because state publication preceded the
IPC bind while the provider was loading. The smoke script now retries only
connection failures during startup, never authorization/inference failures.
The final ONNX run **PASS**ed. Earlier test/build errors are superseded by the
final commands above, not hidden as successes.

### Native operations and restoration

User authorization explicitly permits native control in this continuation.
`cargo run -p aicore-tests --example owned_process_probe --locked --offline`
is a separately invoked project-built probe, not an installer or automatic unit
test. It targets only its own freshly spawned child and rejects inherited
non-Normal classes.

| Target | Modification | Apply/readback | Restoration |
|---|---|---|---|
| Child PID 15228, `owned_process_probe.exe --child` | Normal → AboveNormal (PolicyPriority::High) | **PASS** | Normal restored and read back; cleanup also reported Ok(Normal), child terminated/reaped |
| Child PID 19436, same owned probe | Normal → AboveNormal | **PASS** | Normal restored and read back; cleanup Ok(Normal), child terminated/reaped; `env/validation/owned-process-final.log` |

**Restoration failures: None.** No affinity, system power scheme, GPU/NPU,
registry, service or persistent host configuration was modified. No installer
was run. Test daemons were scratch-path simulator processes and were terminated
by their owning scripts.

The final native probe warms the workload before pairing observations, uses
actual child completion latency and targeted parent+child memory readings through
`ITelemetryProvider::collect_process`, then feeds the real comparison to the
policy wrapper. Endpoint memory delta has MiB resolution and is not a peak-memory
measurement. Final latencies were 3.8752 ms and 3.8956 ms; assessment was
**Inconclusive**, not a verified performance gain. Throughput and energy are
unavailable. No slow AI worker is instantiated in that probe, so it cannot
certify native dual-loop deadlines.

### Environment downloads

Downloaded packages and caches are confined to `env/onnx-packages` and
`env/pip-cache`; pip temporary files used project `env/`. Reused the existing
`C:\Users\12282\anaconda3\python.exe`; no interpreter, Rust toolchain,
Ollama model, installer or driver was downloaded. Python package inventory:

- onnxruntime 1.23.2, onnx 1.20.1, numpy 2.3.5
- coloredlogs 15.0.1, flatbuffers 25.12.19, humanfriendly 10.0
- ml_dtypes 0.6.0, mpmath 1.3.0, packaging 26.3
- protobuf 7.36.2, pyreadline3 3.5.6, sympy 1.14.0, typing_extensions 4.16.0

These are project-local `pip --target` dependencies. The isolated runner does
not import user/global site packages. Generated ONNX model and validation logs
are project artifacts, not downloaded environments.

To resolve the Linux C cross-build blocker, also downloaded the portable
**Zig 0.12.1 Windows x86_64 ZIP** (76,470,574 bytes) from the
[official download index](https://ziglang.org/download/index.json) to
`env/zig-0.12.1-windows.zip`, then extracted into `env/zig-toolchain`.
SHA-256 was verified against the official index:
`52459b147c2de4d7c28f6b1a4b3d571c114e96836bf8e31c953a7d2f5e94251c`.
This is a compiler archive, not an installer. Its global/local cache overrides
are both project-local (`env/zig-global-cache`, `env/zig-local-cache`). No system
PATH was changed. The generated `env/zig-linux-cc.cmd` maps cc-rs's Rust target
spelling to Zig's C target spelling; initial raw/wrapper attempts failed before
the corrected wrapper succeeded. Final cross-check uses:

```powershell
$env:CC_x86_64_unknown_linux_gnu="$PWD\env\zig-linux-cc.cmd"
$env:AR_x86_64_unknown_linux_gnu="$PWD\env\zig-toolchain\zig-windows-x86_64-0.12.1\zig.exe ar"
$env:ZIG_GLOBAL_CACHE_DIR="$PWD\env\zig-global-cache"
$env:ZIG_LOCAL_CACHE_DIR="$PWD\env\zig-local-cache"
cargo check --workspace --all-targets --target x86_64-unknown-linux-gnu --locked --offline
```

This checks current desktop source for Linux compilation. It does not execute
Linux programs, modify WSL, or certify native Linux hardware.

### File integrity and changed files

All 591 baseline tracked files remain. The seven protected residual files'
SHA-256 values are unchanged; before/after inventories and hashes are in
`env/validation/tracked-files-{before,after}.txt` and
`env/validation/residual-hashes-{before,after}.json`. Existing untracked
`docs/assets/` architecture material was preserved. **Deleted original files:
None.** No tests were deleted, ignored or weakened.

Changed/added source, test, model, configuration and documentation paths (not
including environment/cache/build output or pre-existing architecture assets):

```text
Cargo.lock
README.md
adapters/windows/src/telemetry/provider.rs
ai-runtime/src/onnx/mod.rs
ai-runtime/src/onnx/python.rs
ai/Cargo.toml
ai/examples/configured_intent.rs
ai/src/intent.rs
ai/src/intent_config.rs
ai/src/lib.rs
ai/src/orchestration.rs
ai/src/runtime_intent.rs
api/src/identity_policy.rs
apps/aicore-cli/src/main.rs
apps/aicored/src/ipc_handler.rs
apps/aicored/src/runtime.rs
apps/aicored/src/scheduler.rs
contracts/src/execution/targets.rs
contracts/src/ipc/protocol.rs
contracts/src/traits/task_queue.rs
contracts/src/traits/telemetry_provider.rs
core/src/accelerator_scheduler.rs
core/src/executor/policy_executor.rs
core/src/lib.rs
core/src/model.rs
core/src/policy/feedback.rs
core/src/policy/mod.rs
docs/ai/INTENT_BACKENDS.md
docs/contracts/IPC_PROTOCOL.md
docs/quality/DESTINATION_REQUIREMENTS.md
docs/quality/DESTINATION_TEST_SAFETY.md
docs/quality/DESTINATION_VERIFICATION_REPORT.md
docs/runtime/SCHEDULING_AND_FEEDBACK.md
ipc/src/client.rs
ipc/src/identity.rs
ipc/src/server.rs
ipc/src/transport/mod.rs
ipc/src/transport/windows.rs
models/intent_classifier_v1.json
models/intent_classifier_v1.onnx
runtime/src/adaptive_loop.rs
scripts/models/export_intent_onnx.py
scripts/models/onnx_intent_runner.py
scripts/verify/simulated_platform_ollama_intent.ps1
task-runtime/src/queue.rs
tests/Cargo.toml
tests/modelNORealPC.execution.unit.test.rs
tests/ownedProcess.probe.example.rs
```

### Remaining acceptance and blocking dependencies

- **NOT VERIFIED:** native GPU/NPU scheduling, plugin hardware readback and
  thermal/power optimization; no certified device execution/readback backend is
  available. The framework is implemented, but daemon hardware-queue binding
  and real cross-device inference remain dependent on actual backends.
- **NOT VERIFIED:** Windows cross-user ACL denial, signed/loaded image
  attestation, complete stale-peer/reconnect matrix, native hard deadlines,
  real prospective personal-memory accuracy/effect and service lifecycle.
- **NOT VERIFIED:** representative effect/control cohort, workload-specific
  trusted collector integration and energy/peak-memory budgets. Single
  isolated measurements cannot close product effectiveness.
- The initial Linux C compiler blocker is **resolved** by project-local portable
  Zig; the final entire Linux workspace compile and Clippy checks passed. Linux
  native execution/real hardware remains **NOT VERIFIED**.
- **SKIPPED:** unfiltered native-control tests listed above; package installation
  remains prohibited and was not attempted.
- Generic autonomous tool-agent collaboration and generative ONNX LLM adapters
  are not implemented by the classification ensemble. Adding them requires
  defined backends/contracts and a separate architectural scope decision.
- **Pending authorization requests: None** for operations actually performed;
  the explicit user grant covered the owned-child probe. No restoration is
  pending. Installer operations remain forbidden.

This batch passes the Windows code/isolated/model/simulator gates. It does
**not** declare the full destination product or native hardware certification
complete. The existing requirement inventory deliberately remains PARTIAL where
real integrations, representative data or platform acceptance are still missing.

## Continuation: 2026-09-24 (desktop code-layer closure)

This continuation changed only `C:\Users\12282\Desktop\AICore_V4_Windows`.
The WSL project was not synchronized or modified. The desktop build remains
offline with the existing Rust 1.75 toolchain and project-local `env/cargo-home`.
No environment was downloaded, installer or service was started, or host control
setter was invoked.

Application registration can now opt into an operator-owned artifact pin file
(`AICORE_APP_POLICY_PATH`). The daemon resolves a SHA-256 digest from the
authenticated peer's executable with read-only OS process APIs and authorizes
each application action against the pin. Missing, stale, unreadable, or
unapproved identities fail closed. This is an on-disk executable digest and
process-bound identity, **not** signed-image validation or a native Windows
cross-user ACL certification. IPC supports an explicit supported-version
handshake downgrade; unsupported wire major versions still fail.

The existing plugin activation path has typed GPU/NPU/power control messages,
operator pins, live permits, capability checks, and independent readback checks.
Mock tests cover success and fail-closed cases. No real control plugin or
certified readback backend was activated. The local Ollama intent provider uses
`AICORE_OLLAMA_MODEL` (default `llama3.2:3b`) and a loopback-only endpoint; the
IPC intent API returns a validated semantic proposal, not direct actuation.
IPC handler work runs on Tokio's blocking pool so executable hashing and local
model latency do not occupy the async reactor. Model timing observations remain
labelled `Model` and cannot establish real-world effect.

The safe Windows workspace test command excludes five retained tests that can
reach host process or power setters. It passed after the stale Windows GPU test
assertion was corrected to the actual capability contract. The direct Ollama
provider example successfully classified a compilation intent using the
already-running `llama3.2:3b`. An initial simulated-daemon-to-Ollama attempt
failed with connection refused while the external service was unavailable.
After it became available again, the same simulated daemon, Windows named-pipe
IPC, operator-pinned read-only CLI executable SHA-256, and `llama3.2:3b`
intent path **PASS**ed. The result proves that specific
local configuration at the time of test, not other models or real actuation.

| Final desktop check | Result and scope |
|---|---|
| `cargo check --workspace --all-targets --locked --offline` | **PASS**, exit 0 |
| `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings` | **PASS**, exit 0 |
| `cargo fmt --all -- --check` | **PASS**, exit 0 |
| `cargo test --workspace --all-targets --locked --offline -- --skip controller_handles_bad_pid_gracefully --skip executor_runs_and_reports_outcomes --skip full_loop_runs_and_records_metrics --skip history_disabled_by_default_means_no_records --skip history_enabled_records_tick` | **PASS**, exit 0: 141 targets, 711 passed, 0 failed, 0 ignored, 5 filtered; full log in `env/validation/safe-tests-final.log` |
| `scripts/verify/simulated_platform_windows_smoke.ps1` | **PASS**, exit 0: 10 ticks, 5 in-memory successes, 0 failures, named-pipe status/ticks |
| `scripts/verify/simulated_platform_ollama_intent.ps1 -Model llama3.2:3b` | **PASS** on final pinned run, exit 0: read-only executable pin and compilation intent with confidence 1 over named pipe; initial unpinned attempt **FAIL** with connection refused while Ollama was unavailable |

Real workload effect, native Windows ACL cross-user rejection, signed-image
attestation, native control deadlines, certified plugin control readback,
GPU/NPU/power actuation, Windows service/SCM and other model backends remain
**NOT VERIFIED**. They cannot be inferred from simulator or mock success.

## Continuation: 2026-09-24 (Windows desktop Platform Simulator)

The project was copied from WSL to
`C:\Users\12282\Desktop\AICore_V4_Windows` without copying Linux build output.
No installer or service was run. Existing Windows Rust 1.75 and MSVC binaries
were invoked directly. The first offline build found the Windows user's Cargo
cache incomplete (`base64ct` missing); existing local WSL Cargo cache/index
were copied into the desktop project's `env/cargo-home`. All subsequent Cargo
commands used that directory and `--offline`. **No network environment download
was made or approved.**

The new Windows fixture was captured through Windows Adapter read-only methods
and is documented in `docs/platform/PLATFORM_SIMULATOR.md`. Its SHA-256 is
`de6a6903f51c49619059edf4e55e90dd7772a98ddde6c92a953ad3556ffa1cec`.
It contains 10 process identities with getter-observed priority and affinity;
identifying strings are redacted. This host produced no sampled GPU/NPU and no
recognized built-in power profile. Those unsupported paths were not populated
with invented values. Windows builds now embed this fixture; Linux builds keep
their Linux/WSL capture. The `system_smoke` test now uses isolated telemetry and
no controller, asserts `successful + failed + noop = total`, and verifies all
its ticks are neutral NoOp. Its three tests are no longer filtered from the
safe workspace suite. The Windows capability declaration for process priority
and affinity writes is now `Conditional` on target access rights, matching the
actual per-PID getter/setter contract; the stale `Unsupported` test was fixed.

| Check | Result and scope |
|---|---|
| Linux `cargo check --workspace --all-targets --locked` | **PASS**, exit 0 |
| Linux `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | **PASS**, exit 0 |
| Linux `cargo fmt --all -- --check` | **PASS**, exit 0 |
| Linux `bash scripts/verify/model_safe_workspace_tests.sh` | **PASS**, exit 0: 140 test targets, 701 passed, 0 failed, 0 ignored, 40 filtered native-control/host-sensitive tests |
| Windows desktop `cargo check --workspace --all-targets --locked --offline` | **PASS**, exit 0, native MSVC build |
| Windows desktop `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings` | **PASS**, exit 0; Windows-only Clippy defects corrected |
| Windows desktop `cargo fmt --all -- --check` | **PASS**, exit 0 |
| Windows desktop `cargo test -p aicore-adapter-simulated-platform --all-targets --locked --offline` | **PASS**, exit 0: 4 simulator unit tests; no host setters |
| Windows desktop `cargo test -p aicore-daemon simulated_platform_tests --locked --offline -- --test-threads=1` | **PASS**, exit 0: 4 tests covering slow-provider deadline/expiry, readback, rollback and SafeMode against the Windows fixture |
| Windows desktop `cargo test -p aicore-ipc --all-targets --locked --offline` | **PASS**, exit 0: 2 tests, including real same-user named-pipe handshake, Ack and current peer identity |
| Windows desktop `cargo test -p aicore-tests --test system_smoke --locked --offline` | **PASS**, exit 0: 3 isolated NoOp smoke tests |
| Windows desktop read-only `capability_detect_unit` and `telemetry_real_data_unit` targets | **PASS**, exit 0: 8 and 5 tests respectively; no setter invoked |
| Windows desktop `scripts/verify/simulated_platform_windows_smoke.ps1` | **PASS**, exit 0: 10 continuous ticks, 5 successful **in-memory** executions, 0 failed; CLI `status` and `ticks` used a real named pipe with a nonexistent fallback file |
| Unfiltered native-control workspace tests and Windows cross-user ACL denial | **SKIPPED / NOT VERIFIED** under the current authorization and available identity setup |

The Windows same-user named pipe and local peer-identity path now have native
runtime evidence, while Windows service/SCM, cross-user ACL denial, real
priority/affinity/power writes, GPU/NPU control, actual effect and native
control deadlines remain **NOT VERIFIED**. Simulated successes are not native
hardware certification. No system configuration change was requested or made.

## Continuation: 2026-09-24 (captured Platform Simulator)

Destination completion remains **NO / NOT VERIFIED**. This continuation added
`adapters/simulated-platform` and selected it with
`AICORE_EXECUTION_MODE=simulated-platform`. The adapter implements the existing
platform contracts with shared in-memory state, not host setters. Its Linux/WSL
fixture was produced by actual read-only Adapter calls and redacts identifying
strings; see `docs/platform/PLATFORM_SIMULATOR.md` for provenance and SHA-256.
The daemon's continuous Platform Mode uses `PlatformTickDriver` with simulated
telemetry, policy execution, postcondition readback and runtime binding. Plugin
subprocess activation is disabled in this mode. The one-tick daemon path is
still synchronous.

Four daemon integration tests passed for captured PID readback, a 250 ms slow
provider with a prompt neutral fast tick and expired-plan rejection, fresh
prepared-policy application, stale readback causing rollback without a false
success, and repeated write failures entering SafeMode. Three simulator unit
tests passed for affinity restoration, absent hardware/power and fixture
identity checks. `PluginActivationManager::replace_active` now asks the
registry to validate replacement *before* stopping the old process; registry
commit and old-process stop share the registry lock. The existing mock
upgrade/rollback test passed after this correction.

| Check | Result and scope |
|---|---|
| `cargo check --workspace --all-targets --locked` | **PASS**, exit 0, Linux workspace |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | **PASS**, exit 0 |
| `cargo fmt --all -- --check` | **PASS**, exit 0 |
| `bash scripts/verify/model_safe_workspace_tests.sh` | **PASS**, exit 0: 139 targets, 697 passed, 0 failed, 0 ignored, 43 filtered native-control/host tests |
| `cargo test -p aicore-adapter-simulated-platform --all-targets --locked` | **PASS**, exit 0: 3 unit tests, capture example compiles |
| `cargo test -p aicore-daemon simulated_platform_tests -- --test-threads=1` | **PASS**, exit 0: 4 isolated integration tests |
| `bash scripts/verify/simulated_platform_smoke.sh` | **PASS**, exit 0: one `--once` tick and 30 continuous ticks in a temporary directory; final run had 17 successful in-memory executions, 0 failed, remaining ticks neutral/NoOp (an earlier run had 16 successes; timing affects the count) |
| Isolated `AICORE_EXECUTION_MODE=model target/debug/aicored --once ...` | **PASS**, exit 0: local JSON model loaded, one model tick, no host control |
| Read-only Linux telemetry, capability, hardware telemetry and hardware capability targets | **PASS**, exit 0: 20 tests across four targets; the fixture capture was a separate read-only Adapter run |
| `cargo check -p aicore-adapter-simulated-platform -p aicore-plugin --all-targets --locked --target x86_64-pc-windows-msvc` | **PASS**, exit 0: source compilation only |
| `cargo check -p aicore-daemon --all-targets --locked --target x86_64-pc-windows-msvc` | **FAIL**, exit 1 in WSL: bundled `libsqlite3-sys` requires MSVC `lib.exe`, absent here; no Windows daemon behavior inferred |
| `bash scripts/audit_v4_static.sh`, `bash scripts/audit_v32_static.sh`, `bash -n scripts/verify/simulated_platform_smoke.sh`, `git diff --check` | **PASS**, exit 0; V4 audit parsed 110 declared Rust test files |
| Unfiltered `cargo test --workspace --all-targets --locked` | **SKIPPED**: 43 reviewed tests may invoke native controls or host-sensitive paths; tests remain enabled |

The seven named protected residual files retain their pre-change SHA-256
hashes, and `git ls-files --deleted` is empty. The initial full file list was
captured under `/tmp`, but that temporary file was no longer present at final
comparison; therefore a literal whole-tree before/after diff is **NOT
VERIFIED**. No tracked file was deleted. The new crate, fixture, smoke script
and simulator document are additions. No host configuration change was
requested or approved; no installer or service was run. A Windows-local Rust
version probe unexpectedly invoked `rustup`'s partially installed toolchain
repair/download path, so Windows-native build execution was stopped; the WSL
MSVC-target compile result above is the only Windows code evidence here.

The remaining destination gaps are the same substantive product acceptance
gaps in the table below: no actual local LLM model/provider or GPU/NPU model
backend, no operator-approved application artifact/SID policy, no paired
observed workload benefit or policy feedback, no advanced native GPU/NPU/power
actuation evidence, and no native Windows IPC/service or real-control deadline
certification. The WSL fixture has no controllable GPU/NPU or power profile;
simulator success cannot replace those observations. A future real-control
request must list a stable PID/name, target device, exact change, before-state,
scope and rollback and await explicit approval.

## Current continuation: 2026-09-24 (fail-closed input and read-only validation)

Destination completion is **NO / NOT VERIFIED**. The Linux hardware executor
now accepts only `drm:card<digits>` device IDs and known workload hints.
Unknown hints and power caps return NotSupported before any GPU write. Linux
and Windows power adapters no longer interpret an unfamiliar OS power scheme
as Balanced: readback reports CapabilityUnavailable. The typed plugin priority
bridge rejects PID 0 before sending a request, because PID 0 has special
process-control meaning on some platforms. These are contract/safety fixes;
they add no product capability. Linux IPC now also captures the peer
executable device/inode and rechecks it on every application-scoped request.
A replaced executable loses authorization, while an unreadable identity is
denied for that request without transferring ownership to another peer.
This is a process-artifact continuity check, not an operator-approved
application signing/SID policy. An isolated observed-workload helper now
measures successful caller-owned work with a monotonic clock and leaves
throughput/energy absent. Its CPU fixture is an actual local computation,
but no policy intervention or paired control was run; benefit remains
unverified. Personal-memory consent/retention and SQLite tests used only
in-memory or isolated fixtures, not personal user data.

| Current check | Result and scope |
|---|---|
| `cargo test -p aicore-adapter-linux --lib --locked` | PASS, exit 0: 5 tests, including three new pure fail-closed tests; no control call |
| `cargo test -p aicore-plugin activation::tests::typed_priority_control_requires_exact_operator_pin_and_fails_closed --locked` | PASS, exit 0: 1 mock test; PID 0 causes no host request |
| `cargo check -p aicore-adapter-windows --all-targets --locked --offline --target x86_64-pc-windows-msvc` | PASS, exit 0: code compilation only |
| `cargo test -p aicore-ipc --lib --locked` | PASS, exit 0: 9 tests, including changed-executable identity and per-request rejection |
| `cargo test -p aicore-daemon --bin aicored --locked ipc_handler::tests::` | PASS, exit 0: 6 isolated IPC task/owner tests |
| `cargo test -p aicore-core effect::tests:: --locked` | PASS, exit 0: 4 effect tests, including a measured isolated CPU workload and failed-workload rejection |
| `cargo test -p aicore-memory personal::tests --locked` plus SQLite/retention targets | PASS, exit 0: 15 consent, retention and store tests using isolated data |
| `cargo check --workspace --all-targets --locked` | PASS, exit 0 |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | PASS, exit 0 |
| `cargo fmt --all -- --check` | PASS, exit 0 |
| `bash scripts/verify/model_safe_workspace_tests.sh` | PASS, exit 0: 137 targets, 690 passed, 0 failed, 0 ignored, 43 filtered after all source changes |
| `AICORE_EXECUTION_MODE=model cargo run -p aicore-daemon --bin aicored --locked -- --once ...` | PASS, exit 0: local JSON model loaded, 7 simulated outcomes in the latest isolated run; no hardware control |
| Selected read-only Linux capability, telemetry, hardware and power-query targets | PASS: 21 tests across five targets; no platform setter invoked |
| Real Platform Mode control tick, native Windows pipe and real benefit | SKIPPED / NOT VERIFIED; see preflight below |

Platform control preflight: **no approval requested and no host configuration
changed**. A normal Platform Mode tick may call `setpriority` for detected
foreground PIDs, `sched_setaffinity` for foreground/background PIDs if a
policy provides a core mask, `setpriority(..., nice=19)` for throttled
background PIDs, Linux power-profile sysfs writes, and AMD GPU performance
sysfs writes if available. This WSL host currently has no DRM card, ACPI
platform-profile or CPU governor path, but process targets come from a live
telemetry snapshot and can change between ticks. No stable, isolated PID/name
has been selected, so the user's exact-target approval condition is not yet
met. A controlled test must first freeze an isolated target PID and its
baseline priority/affinity; it must also capture any present device and power
state, specify requested values, scope and rollback, then request explicit
approval before invoking a setter. Existing policy rollback is not evidence
that the host's exact original settings were restored. Platform Mode deadline
and effect acceptance remain open.


## Current continuation: 2026-09-24 (read-only native evidence)

Destination completion remains **NO / NOT VERIFIED**. This continuation used
the newer authorization for read-only Linux host observation. It did not run
an installer, service or platform control tick.

- Continuous Platform Mode now uses a bounded slow preparation worker. Its
  telemetry/history/prediction/policy path no longer blocks polling; the
  platform Binder stays after the plan expiry and SafeMode check because it
  can write GPU/NPU state. Mock tests show a 250 ms slow telemetry call does
  not block a fast poll and an expired plan never invokes the Binder. The
  platform Binder, executor, event publishing and state file can still delay
  the execution side; no hard deadline or real-control pass is claimed.
- Linux process priority, affinity, power-profile and AMD GPU write capability
  declarations now state the required runtime permission/driver conditions.
  A present sysfs path alone is not reported as unconditional write support.
  CPU/AI Binder routes without inference input now report `executed=false`;
  GPU/NPU device IDs have no synthetic OS-specific fallback.
- A periodic daemon reaper checks confirmed dead/reused IPC peers on a blocking
  worker, cancels their active tasks and clears L1/registry bindings. The
  prepared-policy apply path rechecks task revocation before binding. This
  remains process identity, not operator-approved application artifact identity.
- Plugin `process.priority_v1` now has a typed request/response and an exact
  `Adapter`/`process.control`/operator `allow_control` pin gate. Platform Mode
  binds a unique active plugin through `ProcessSelection`; its failure does
  not trigger a native write. The independent platform controller supplies
  postcondition readback. This path has mock tests only; no external control
  plugin was executed and GPU/NPU/power plugin control remains unbound.
- Read-only Linux adapter, hardware capability and telemetry/context tests
  passed on this WSL host. These confirm readings are available and structurally
  valid; they are not independent sensor calibration. A Model Mode `--once`
  run loaded the local JSON model and applied seven simulated outcomes.
- Source review confirmed the existing isolation filter already excluded the
  GPU `release()` test, which may write AMD sysfs. A read-only check found no
  corresponding GPU write path on this WSL host. No real-device control result
  is inferred from the filtered run.

| Check in this continuation | Result and scope |
|---|---|
| `cargo check --workspace --all-targets --locked` | PASS, exit 0, Linux workspace |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | PASS, exit 0, Linux workspace |
| `cargo fmt --all -- --check` | PASS, exit 0 after formatting the new capability assertion |
| `bash scripts/verify/model_safe_workspace_tests.sh` | PASS, exit 0: 137 targets, 684 passed, 0 failed, 0 ignored, 43 filtered. The pre-existing filter already excluded GPU `release()` |
| `cargo test -p aicore-runtime dual_loop --locked` | PASS: 7 targeted tests, including Platform Mode slow-provider and expiry mocks |
| `cargo test -p aicore-daemon --bin aicored --locked ipc_handler::tests::` | PASS: 6 targeted tests, including stale-owner task cleanup |
| Plugin activation and daemon process-selection mock tests | PASS: 8 plugin activation tests and 1 selection fail-closed test; no real process priority changed |
| Linux read-only adapter, hardware and capability targets | PASS: 27 selected tests across eight separately invoked targets; no controller call |
| `cargo check -p aicore-ipc -p aicore-contracts -p aicore-plugin --all-targets --locked --offline --target x86_64-pc-windows-msvc` | PASS, exit 0: Windows-target code compilation only; no native pipe runtime evidence |
| `AICORE_EXECUTION_MODE=model cargo run -p aicore-daemon --bin aicored --locked -- --once ...` | PASS, exit 0, local JSON model loaded; 7 **simulated** outcomes; no host control |
| Unfiltered workspace `cargo test` and real Platform Mode tick | SKIPPED: direct process, GPU or power control may change host configuration; separate authorization required |
| Native Windows named-pipe runtime | NOT VERIFIED: local Rust 1.75 toolchain was absent; an offline check began a toolchain download and was immediately interrupted; no test binary or installer was run |
| `bash scripts/audit_v4_static.sh`, `bash scripts/audit_v32_static.sh`, shell `bash -n`, `git diff --check` | PASS, exit 0; 110 declared Rust test files and 18 discovered shell scripts |
| Seven protected residual SHA-256 hashes and before/after file manifest | PASS: all hashes unchanged; no pre-existing non-`target/` file removed or new source file added |

The earlier section below records the prior code-only continuation and must
not be read as a current Windows or hardware certification. Remaining items
include independently verified plugin control postconditions, operator-pinned
application artifacts, paired real workload effect observations, local LLM
and GPU/NPU model backends, native Windows IPC, and measured Platform Mode
deadline/rollback. These are destination work, not V4 defect closure claims.

No system configuration change was authorized or deliberately executed in this
continuation. A real Platform Mode tick is still **SKIPPED** pending explicit
permission because its existing adapters may change process priority/affinity,
power profile or GPU performance state. Native Windows named-pipe runtime is
**NOT VERIFIED**; Linux/WSL read-only observation does not substitute for it.
An attempted Windows-host offline compile triggered rustup's missing Rust 1.75
component download and was immediately interrupted; WSL Windows-target code
compilation subsequently passed without running Windows binaries.

## Latest continuation: 2026-09-24 (Model Mode/code only)

Destination completion remains **NO / NOT VERIFIED**. This continuation
advances several code paths but does not replace product or hardware acceptance.

- Continuous Model Mode daemon scheduling now uses one bounded slow preparation
  worker and a fresh validated policy handoff. The slow side owns telemetry,
  historical reads, prediction and model runtime binding; the fast side applies
  model execution, verification and SafeMode, or records a neutral NoOp.
  Discarded plans fail dequeued tasks rather than leave them Running. A mock
  250 ms telemetry provider did not block the fast call; an isolated daemon
  test applied a prepared policy and shut down. `--once` and Platform Mode
  retain the synchronous path. Actual fast/slow durations are separately
  measured in microseconds, but these are **not** performance benefits.
- Model Mode retention runs on a bounded background worker. The scheduler's
  retention timestamp was corrected from seconds to the scheduler's millisecond
  contract. NoOp no longer increments runtime failure metrics.
- A typed read-only `telemetry.snapshot_v1` plugin binding requires a Telemetry
  manifest, `telemetry.read` permission and an operator pin. Explicit restart
  after failure and staged `replace_active` support mock-tested upgrade and
  rollback: a failed candidate leaves the old process active. Platform Mode
  selects a unique active typed telemetry plugin and falls back to platform
  observation on failure. Model Mode never starts it; daemon selection has
  mock/code evidence only.
- IPC registration can reclaim a confirmed dead/reused process binding and
  clear its L1 declaration (Linux mock/runtime evidence, Windows code compile
  only). The bound identity remains process-level;
  operator-approved application artifact identity and queued-task cleanup are
  still missing. A handshake can explicitly negotiate the sole implemented
  1.0 protocol; later versions are not operationally supported.
- Opt-in history can store an explicit registered application ID. Aggregation
  derives same-app/UTC-hour patterns after at least three samples; prediction
  consumes them only with a matching task identity. Old records deserialize
  without the new field. A mock integration test covers the resulting policy
  path and history record. This is not prospective scheduling benefit.
- The daemon enables the conservative thermal/power guard. The shared local
  JSON model is registered as a CPU entry in `DeviceRuntimeCatalog` and used
  by the Model Mode binder. Platform Mode populates read-only vendor/OS matrix
  declarations from detected devices; these are not actuator proof. No GPU/NPU model backend or local LLM model is
  installed or claimed. The effect comparison framework still has no paired
  observed workload/energy source, so it cannot claim improvement.

| Current check | Result and scope |
|---|---|
| `cargo check --workspace --all-targets --locked` | PASS, exit 0, Linux workspace |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | PASS, exit 0, Linux workspace |
| `bash scripts/verify/model_safe_workspace_tests.sh` | PASS, exit 0: 137 targets, 679 passed, 0 failed, 0 ignored, 43 filtered by the reviewed live-control/host list |
| `AICORE_EXECUTION_MODE=model cargo run -p aicore-daemon --bin aicored --locked -- --once ...` | PASS, exit 0: one model tick, seven simulated outcomes, native route and no runtime binding action; local JSON model loaded |
| Isolated continuous daemon Model Mode test | PASS: prepared policy applied and daemon shut down; this is not a native service test |
| `cargo check -p aicore-contracts -p aicore-core -p aicore-ipc -p aicore-plugin -p aicore-ai-runtime --all-targets --locked --target x86_64-pc-windows-msvc` | PASS, exit 0; code compilation only |
| Windows target including `aicore-runtime` | FAIL in WSL: bundled SQLite build cannot find MSVC `lib.exe`; no Windows runtime conclusion |
| `bash scripts/audit_v4_static.sh`, `bash scripts/audit_v32_static.sh`, shell `bash -n` | PASS, exit 0 |
| Unfiltered `cargo test --workspace --all-targets --locked` | SKIPPED: 43 listed tests can use live controls/host capabilities |
| Real hardware, service/package installation and native Windows behavior | NOT VERIFIED by task boundary |

The latest continuation touched the files in the final working-tree diff,
including `apps/aicored`, `runtime`, `plugin`, `ipc`, `contracts`, `core`,
`memory`, `tests` and this documentation. No source or protected residual was
deleted. The before/after file manifests under `/tmp/aicore-destination3-files-*.txt`
showed no removed or added non-`target/` paths; Cargo replaced 4,607 generated
`target/` paths. The seven protected residual SHA-256 checks all passed.
`cargo fmt --all -- --check`, shell syntax, `git diff --check` and static
audits exited 0 in the final regression.

### Remaining code and acceptance gaps

The typed plugin telemetry path still needs native process evidence and a
safe control-interface design; automatic restart/update policy is unimplemented.
Application identity needs an operator-approved executable/signing policy,
Windows native cleanup and operational compatibility beyond the single 1.0
schema. Effect assessment needs independently observed paired workload metrics
before feedback can influence policy. There is no local LLM model/runtime,
intent API or GPU/NPU inference provider. The populated vendor/OS matrix is
not yet bound to safe actuators or verified on target hosts. Fast
execution/event/state writes and Platform Mode do not have a demonstrated
deadline. No measured personalization, thermal or power benefit exists. These
remain open destination requirements, not V4 defect claims.

## Prior batch record (2026-09-23)

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
