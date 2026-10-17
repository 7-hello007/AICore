[English](P0_FIX_VERIFICATION_REPORT.md) | [中文](P0_FIX_VERIFICATION_REPORT.zh-CN.md)

# P0 repair and verification — 2026-10-17

## Scope and evidence

Sole checkout: `C:\Users\12282\Desktop\AICore`. Baseline: `9002c6e4499dcbed7543f413ce2d47511e56c962`; working branch: `codex/p0-defect-fix`. No commit or other checkout synchronization was performed.

The preceding audit in this session inventoried 708 tracked files, read all 702 text files (70,240 lines), and inspected metadata/relationships for six binary/generated assets. Git was initially clean; no applicable AGENTS.md was found. Historical reports were investigation leads, not current PASS evidence. New/changed code, assertions and call sites were reviewed during repair. Binary assets were not claimed as line-by-line reads.

Current logs, command ledger and integrity inventories are in `C:\Users\12282\.codex\visualizations\2026\10\16\01a143bb-0c55-7331-af19-38acccd660f4\aicore-p0`. Earlier reading coverage is in the adjacent `aicore-audit` directory. This report distinguishes test assertions, portable fixtures, models and native Windows process checks from physical-device certification.

The October 17 continuation preserved the existing diff and reviewed changed files, direct dependencies and retained evidence; it did not restart the audit. All continuation work was performed by the main agent without subagents or delegation. Fresh final logs are in the `closure-2026-10-17` subdirectory. Earlier RED/component results below are explicitly earlier stages of this repair, not newly executed PASS results.

## Repair decisions

| ID | Cause and resulting behavior | Principal source evidence | Decision / verification boundary |
| --- | --- | --- | --- |
| C01 | An L1 task and dequeued task could supply different identity/context/constraints in one tick. Select one contract consistently; leave queue work queued while L1 owns the tick. | `runtime/src/adaptive_loop.rs`, task-pipeline integration tests | FIXED; real queue + conflicting contract regressions |
| C02 | Cleanup was disarmed before revocation/deadline/read errors or despite failed terminal writes. Retain guard until transition succeeds; reread and fail only Running work, with two bounded cleanup attempts. Concurrent terminal states survive. | `runtime/src/adaptive_loop.rs`, task-pipeline integration tests | FIXED; timeout/cancel/drop/read-error/write-error regressions. Permanently unavailable storage remains an infrastructure limitation. |
| C03 | Prior policy replay and partial success falsely implied rollback. Default executor captures originals before any write; transaction spans apply/readback/restore; restore all attempted resources and finally verify the complete saved state. Failed requests remain AllFailed even after successful recovery. | `contracts/src/traits/policy_executor.rs`, `core/src/executor/{policy_executor,transaction,transparent}.rs`, `runtime/src/adaptive_loop.rs` | FIXED; fault-injected exact-state, false ACK, coupled restore, concurrency and SafeMode tests |
| C04 | Router/plugin paths could invoke raw GPU/NPU executors. Router requires an explicit guarded actuator + permit; plugin hardware controls use the safe actuator hook; legacy raw bindings cannot mutate. Native backends without reversible state reject before writes. | `runtime-router/src/binder.rs`, `core/src/safe_actuator.rs`, `plugin/src/activation.rs`, daemon wiring | FIXED; actual guard/router with stateful model boundaries; physical controls NOT VERIFIED |
| C05 | Blocking stdin writes preceded timeout, under the process map lock. Per-child workers/operation locks budget queue + complete write + response; unrelated plugins progress; cancellation kills/reaps and joins workers. Windows cancels inherited overlapped pipes too. | `plugin/src/host/process.rs`, `plugin/tests/process_deadlines.rs` | FIXED; 11 real subprocess regressions; Unix cancellation runtime NOT VERIFIED |
| H01 | ID prefixes/display vendor strings supplied authority; ordinals changed identity. Explicit kind, OS numeric vendor, Windows DXGI LUID/NVIDIA UUID and Linux canonical sysfs identities replace those assumptions. Session ordinals resolve through DXGI enumeration. | hardware contracts, Windows/Linux hardware telemetry/capability providers, `apps/aicored/src/runtime.rs` | FIXED; identity/spoofing fixtures; broad device matrix NOT VERIFIED |
| H02 | Discovery/path/tool presence advertised controls/telemetry as available. General GPU providers and native executors now agree on unsupported reversible control. Generic accelerator discovery does not invent telemetry. Applied and independently Verified differ; model results cannot become native Verified. | platform capability/GPU providers, NPU telemetry, hardware status contracts and guard | FIXED; discovery-only and forged-model-Verified regressions |
| H03 | Command acknowledgement implied effective hardware state. Guard independently captures/reads requested state, restores on partial failure/mismatch/expiry, then reads originals again. | `core/src/safe_actuator.rs`, hardware executor state hooks, simulator | FIXED; readback/partial-write/restore-error/expiry/concurrency faults; physical readback NOT VERIFIED |
| H04 | Coarse priority/power buckets and auto reset lost native originals. Preserve Windows raw class/creation time, Linux exact nice/start time, Windows GUID, Linux custom profile/per-CPU governors. Bind process writes/restores to captured identity; refuse irreversible priority changes. Exact power recovery explicitly opts in, and Core/plugin share the resource lock. | process/power contracts and adapters, policy transaction, plugin activation | FIXED; portable/raw-state and safe Windows process tests. Native Linux and physical GPU/power recovery NOT VERIFIED. |

No old test was removed, ignored or weakened. Old assertions equating policy replay with recovery were replaced by stronger original-value assertions. Recovery success is recorded separately from request failure; this prevents NoOp from suppressing task failure and SafeMode. Existing callback implementations remain source compatible through additive trait defaults; the added `Verified` enum variant requires external exhaustive matches to handle it.

### Source and regression locations

All line numbers below refer to the final working tree. The complete changed-file list is retained with the external diff evidence.

| ID | Source location | Executed regression location |
| --- | --- | --- |
| C01 | [Single selected task contract](C:/Users/12282/Desktop/AICore/runtime/src/adaptive_loop.rs:644) | [L1/queue conflict](C:/Users/12282/Desktop/AICore/tests/pipeline.taskPipeline.integration.test.rs:88) |
| C02 | [Bounded terminal cleanup](C:/Users/12282/Desktop/AICore/runtime/src/adaptive_loop.rs:79) | [Terminal-write faults and concurrent cancellation](C:/Users/12282/Desktop/AICore/tests/pipeline.taskPipeline.integration.test.rs:495) |
| C03 | [Original capture](C:/Users/12282/Desktop/AICore/core/src/executor/transaction.rs:42), [restore and final readback](C:/Users/12282/Desktop/AICore/core/src/executor/transaction.rs:147), [failure classification](C:/Users/12282/Desktop/AICore/core/src/executor/transparent.rs:194) | [Shared transaction concurrency](C:/Users/12282/Desktop/AICore/tests/transparent.control.unit.test.rs:400), [coupled final readback](C:/Users/12282/Desktop/AICore/tests/transparent.control.unit.test.rs:470) |
| C04 | [Authorized router](C:/Users/12282/Desktop/AICore/runtime-router/src/binder.rs:300), [plugin entry](C:/Users/12282/Desktop/AICore/plugin/src/activation.rs:506) | [Guard identity mismatch](C:/Users/12282/Desktop/AICore/core/src/safe_actuator.rs:635), GPU/NPU/router integration targets |
| C05 | [Complete call deadline](C:/Users/12282/Desktop/AICore/plugin/src/host/process.rs:90) | [Blocked-write timeout](C:/Users/12282/Desktop/AICore/plugin/tests/process_deadlines.rs:153), [cleanup cycles](C:/Users/12282/Desktop/AICore/plugin/tests/process_deadlines.rs:471) |
| H01 | [Numeric vendor authority](C:/Users/12282/Desktop/AICore/contracts/src/hardware/matrix.rs:25), [ordinal mapping](C:/Users/12282/Desktop/AICore/adapters/windows/src/hardware/telemetry.rs:13) | [Display vendor spoof](C:/Users/12282/Desktop/AICore/contracts/src/hardware/matrix.rs:108), [ID prefix rejection](C:/Users/12282/Desktop/AICore/contracts/src/hardware/capability.rs:161) |
| H02 | [Distinct Verified status](C:/Users/12282/Desktop/AICore/contracts/src/traits/hardware_executor.rs:27) | [Model cannot forge native Verified](C:/Users/12282/Desktop/AICore/core/src/safe_actuator.rs:464), `p0_linux_hardware` discovery/capability fixtures |
| H03 | [Guarded capture/apply/readback/recovery](C:/Users/12282/Desktop/AICore/core/src/safe_actuator.rs:34) | [Independent verification](C:/Users/12282/Desktop/AICore/core/src/safe_actuator.rs:447), [mismatch](C:/Users/12282/Desktop/AICore/core/src/safe_actuator.rs:578), [restore failure](C:/Users/12282/Desktop/AICore/core/src/safe_actuator.rs:607) |
| H04 | [Exact power gate](C:/Users/12282/Desktop/AICore/plugin/src/activation.rs:587), Windows/Linux process and power controllers | [Windows lifetime/REALTIME refusal](C:/Users/12282/Desktop/AICore/adapters/windows/src/process/controller.rs:522), [exact mixed Linux governors](C:/Users/12282/Desktop/AICore/adapters/windows/tests/linux_power_recovery.rs:59), [legacy coarse power refused](C:/Users/12282/Desktop/AICore/tests/transparent.control.unit.test.rs:631) |

## Test-first and component evidence

All Cargo commands below use the installed 1.75 MSVC `cargo.exe` and matching `rustc.exe`/PATH, with `CARGO_NET_OFFLINE=true`, `--locked --offline`. Process environment changes apply only to the invoked command.

| Evidence / command | Actual result |
| --- | --- |
| `cargo test -p aicore-tests --test pipeline_task_pipeline_integration ...` before task fixes | RED: 3 original failures; follow-up terminal-write fault cases: 4 failures (`task-red*.log`, `task-finish-red.log`) |
| Task pipeline + adaptive loop + queue + lifecycle targets | GREEN: 47 passed, 0 failed after terminal-write fixes (`task-finish-green.log`); earlier 83 selected passes are explicitly earlier-stage evidence |
| `cargo test -p aicore-tests --test transparent_control_unit p0_ ...` before recovery fixes | RED: 5 failures / 1 pass. Additional RED logs cover shared-wrapper interleaving, coupled restoration and legacy coarse power capture. |
| `cargo test -p aicore-tests --test transparent_control_unit --test rollback_integration ...` | Selected recovery results and subsequent complete workspace results are recorded in the command ledger; final assertions include original-value recovery, complete final readback and failure classification. |
| Windows process lifetime + REALTIME reversibility tests | RED observed separately; forged lifetime tests request only the current mask/priority even in RED. GREEN package evidence: 7 then 9 native-safe units plus 16 portable Linux tests. No actual REALTIME process was created. |
| `cargo test -p aicore-plugin --test process_deadlines ...` | RED: 4 pass / 4 fail; final GREEN: 11 pass / 0 fail. Twelve cleanup cycles retained 127 handles / 5 threads after warmup. |
| Hardware contracts/Core/router/simulator/plugin + guarded model integration | Separate RED logs cover bypass, discovery, false ACK, partial restore, expiry, model status and release. Latest review run: 40 library + 3 portable Linux tests PASS. Earlier selected guarded route integration: 17 PASS. Counts overlap; do not sum these into a workspace total. |
| First workspace attempt | FAIL: 3 daemon assertions exposed unavailable optional power blocking CPU controls and recovered failures becoming NoOp. Both causes were fixed without suppressing assertions. |
| First post-correction daemon rerun | 10 pass / 1 fail exposed remaining runtime NoOp override; removed that override and retained failure accounting. |

Portable Linux tests compile actual portable Linux modules and use temporary files/synthetic process state on Windows. The harness is not native Linux execution. Tests with historic `gpuReal`, `npuReal`, `realModel` names use the boundaries shown in their source; no native LLM, ONNX, GPU or NPU inference is claimed from those names.

## Final engineering gates

These commands were actually executed on October 17. A later formatting-only correction joined one expression in `core/src/safe_actuator.rs`; its failed initial fmt log and successful recheck are both retained. No semantic source change followed the successful test/Check/Clippy gates.

| Command | Final result | Evidence in `closure-2026-10-17` |
| --- | --- | --- |
| `cargo test -p aicore-tests --test pipeline_task_pipeline_integration --test rollback_integration --test runtime_safe_mode_unit --test task_lifecycle_unit --test task_queue_unit --test transparent_control_unit --locked --offline -- --test-threads=1` | PASS: 67 passed, 0 failed, 0 ignored, 0 filtered; 6 targets | `targeted-control.log`, `targeted-exits.json` |
| `cargo test -p aicore-plugin --all-targets --locked --offline -- --test-threads=1` | PASS: 22 passed, 0 failed (11 library + 11 real subprocess tests); former 8/1 package failure resolved | `plugin-package.log`, `targeted-exits.json` |
| `cargo test --workspace --all-targets --locked --offline -- --test-threads=1 --skip executor_runs_and_reports_outcomes` | PASS for executed scope: 809 passed, 0 failed, 0 ignored, 1 filtered; 150 targets; exit 0 | `workspace-tests.log`, `workspace-test-exit.json`, `workspace-target-counts.csv` |
| `cargo check --workspace --all-targets --locked --offline` | PASS, exit 0 | `cargo-check.log`, matching exit JSON |
| `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings` | PASS, exit 0 | `cargo-clippy.log`, matching exit JSON |
| `cargo check -p aicore-adapter-linux --all-targets --target x86_64-unknown-linux-gnu --locked --offline` | PASS, exit 0; Linux-target compilation, not execution | `linux-check.log`, matching exit JSON |
| `cargo clippy -p aicore-adapter-linux --all-targets --target x86_64-unknown-linux-gnu --locked --offline -- -D warnings` | PASS, exit 0; Linux-target lint only | `linux-clippy.log`, matching exit JSON |
| `cargo fmt --all -- --check` | Initial FAIL: one line-wrapping difference; final PASS, exit 0 | `cargo-fmt.log`, `cargo-fmt-final.log`, both exit JSON files |
| `git -c core.safecrlf=false diff --check` | PASS, exit 0; command-scoped Git override only | Final integrity ledger |

Counts overlap across targeted and workspace runs and must not be summed as unique coverage. Cargo's 37 unit/example harnesses report 109 passes; its 113 separate integration targets report 700 passes and the one safety filter (`workspace-harness-counts.json`). Workspace test harnesses also linked all tested targets; an additional redundant `--no-run` gate was unnecessary. Within the 809 passes, actual guarded GPU/NPU model integration has 5/2 passes, router binding 10, task pipeline 20, SafeMode 9, portable Linux power/process fixtures 16 and portable Linux hardware fixtures 3. Native Windows checks operate on captured/self-owned processes with restoration/readback; they do not certify global power or physical accelerators. Tests for installation/service behavior use inspected mocks; no installation path was run.

The workspace test command intentionally filters exactly `executor_runs_and_reports_outcomes`, whose optional power fixture changes a shared global OS power scheme. The test remains intact. No fully unfiltered workspace test PASS is claimed. Ordinary test executables and self-owned child processes are allowed; no installer or service installation was used.

## Environment and blocked validation

| Environment/dependency | Version / use | Local result / blocking impact | Acquisition decision |
| --- | --- | --- | --- |
| Rust/Cargo/Clippy/rustfmt | Installed Rust 1.75 x86_64 Windows MSVC | Reused directly; Windows builds/tests link with existing MSVC/SDK | No download needed |
| Windows C++/SDK | Existing VS2022 MSVC 14.44, SDK 10.0.26100 | Actual Cargo linking exercises installed tools; no installation | No download needed |
| Linux target dependency | `windows_x86_64_gnu 0.52.6`, selected by offline cross-target resolution | Initially absent from the default cache; continuation found its existing source/archive in project `env/cargo-home`. Offline metadata, Linux-target Check and Clippy now PASS using that cache. Linux native runtime remains NOT VERIFIED. | No download needed or attempted; existing cache reused through command-local CARGO_HOME |
| ONNX/Python/LLM/model downloads | Not required for these fixes or model/fault boundaries | No missing ONNX package is treated as a reason to install; no model inference claimed | None requested |
| Real hardware, installer/service matrix | Separate native/product acceptance | Native GPU/NPU/global power tests SKIPPED; Linux/cross-vendor certification NOT VERIFIED; package/service registration SKIPPED | Requires separately safe isolated native acceptance; no installation authorized |

## Safety and integrity

Final tracked-plus-untracked inventory contains 716 files: all original 708 plus eight intentional new files. No original file is missing; continuation inventory remains unchanged. The normalized tracked semantic diff contains 42 files (4,576 insertions / 1,157 deletions before the final one-expression formatting adjustment); eight new files add the transaction implementation, three fixture/report groups and the subprocess suite. Broad pre-existing line-ending changes remain separately visible; they were not repeated or reverted in the continuation.

All six protected files match the original SHA-256 hashes exactly. `protected-final.csv` records both hashes; `hashes-after.csv` and `integrity.json` preserve the inventory. Final process inspection found zero matching deadline/native-test or owned sleep-child leftovers (`test-processes-final.json`); the subprocess leak regression also passed in the fresh run. No commit, branch switch, cleanup or other-copy modification occurred.

At continuation entry, `env/` already existed (directory creation timestamp October 16, 19:40:13), after the earlier 18:43 check recorded its absence. It contains historical caches and validation assets. Its origin is NOT VERIFIED: that timestamp alone cannot establish who created or populated it. It was preserved; no continuation action created it or downloaded assets. Linux validation reused the already-present dependency cache. This discrepancy is recorded instead of silently asserting that env is still absent.

| Protected file | SHA-256 (before = after) |
| --- | --- |
| `apps/aicore-dashboard/src/client.r` | `E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855` |
| `apps/aicored/src/runtime.rs.realPC` | `C0F2C8282D7607491DF697585BA73A6BB12B05C5F0DFC6EE6582E96E8C6B3593` |
| `apps/aicored/src/scheduler.rs.realPC` | `49FE22A93BB00F23EAB844EB3D4A80443935A50BF33DEB383902D73378A6FA16` |
| `core/src/lib.rs.realPC` | `BEC54D958926F04FE514CF82B9BE1B89CED46EBB208418BD8023DFAD5F60FD59` |
| `runtime/src/lib.rs.realPC` | `94A04FAFA04519F36FE39D58D06E1ACFB2385B6C298B1CDA06E7FD2B9ABE6D81` |
| `tests/Cargo.toml.model.should.delete` | `560DFB7DE492AD78DBD79B4BAFA0EA58154E7D6F13F19270F7B2AED4280C869C` |

No downloads, dependency/toolchain/component installation, model pull, installer execution, service registration, system environment-variable changes, or other persistent system configuration changes were performed. Existing Rust/cache/MSVC were reused. The named missing output remains absent; it was not reconstructed. Build artifacts under target and the external evidence folder are intentional retained outputs.

Existing CRLF active Rust files conflicted with the repository's Unix rustfmt rule before repair. `cargo fmt --all` normalized active Rust inputs and fixed the existing adaptive-loop comment indentation; protected residual extensions were not formatted. Git's normalized semantic diff and byte-hash inventory are separate evidence.

## Remaining limits

- Native Linux POSIX priority/affinity APIs address numeric PIDs; saved lifetime checks bracket mutations, but the kernel check/syscall race is not eliminated. Windows uses the same owned handle for lifetime validation and mutation.
- No process can guarantee terminal persistence while queue storage remains permanently unavailable. Cleanup retries are bounded and never overwrite a concurrent terminal state.
- Shared transaction locks serialize participating AICore controllers, not arbitrary external programs changing the same OS resources.
- GPU/NPU built-ins lacking exact reversible native state refuse control; successful model paths remain usable and explicitly model-only. Physical control, native readback, recovery authority and successful-operation release require provider-specific native acceptance.
- No claims of application benefit, energy improvement, hard real-time certification, cross-hardware support or product certification follow from these tests.

All nine P0s are FIXED at the source/regression level, with the applicable offline engineering gates passing. P0 engineering closure is achieved within the authorized safe test scope; an unfiltered workspace run and complete native hardware/platform acceptance remain incomplete. No remaining executed test failure or missing download blocks the repaired paths. Physical GPU/NPU/power controls remain unavailable where exact reversible providers are absent; Linux native runtime, Unix pipe cancellation and cross-vendor certification require separately bounded native acceptance. Their absence is not reported as PASS.
