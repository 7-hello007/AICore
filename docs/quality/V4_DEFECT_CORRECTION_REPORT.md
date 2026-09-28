[English](V4_DEFECT_CORRECTION_REPORT.md) | [Chinese](V4_DEFECT_CORRECTION_REPORT.zh-CN.md)
# AICore V4 Final Engineering Closure Report

Validation date: 2026-09-23. Version 0.4.0; WSL Ubuntu, Rust/Cargo 1.75.0, nextest 0.9.85. This is historical evidence of code-level engineering closure, not production deployment, native Windows operation, or cross-hardware certification.

## 1. Final Status

| Item | Result |
|---|---|
| V4 Engineering Closure | PASS (Code Layer) |
| Build | PASS: workspace all-targets check/build |
| Clippy | PASS: workspace all-targets, all-features, -D warnings |
| Full cargo test | PASS: 666 passed, 0 failed / ignored / filtered |
| Full nextest | PASS: 666 run, 666 passed, 0 skipped; 31.223s |
| Model Mode | PASS: isolated daemon --once, combined model tests, and archive-binary smoke test |
| Linux local tests | PASS: included nine previously unrun local-control tests |
| Windows compile verification | Targeted service-host/adapter/IPC PASS; full workspace NOT VERIFIED because WSL lacked MSVC lib.exe |
| Packaging build verification | tar.gz / deb PASS; content and checksums checked; neither installed |
| Docs | PASS: status, operations guide, plugin boundary, and release gate synchronized |
| Residual files | PASS: SHA-256 of seven specified files unchanged; no original files deleted |
| Can enter next stage | YES for code engineering only; production certification was not established |

## 2. Previous V4 Blockers

| Previous issue | Source review, resolution, and evidence | Final status |
|---|---|---|
| Full tests had not run | No global Model Mode override was set. Both cargo test and nextest ran without filters; nextest passed 666/666. | CLOSED |
| One versus eight tasks per tick | AdaptiveLoop builds one policy, validation, and execution result per tick and currently dequeues one task. Contracts promise no eight-task throughput. Raising only the constant would wrongly assign one result to several tasks. Keep one task, strengthen per-tick assertions, and align the docs. | CLOSED: one task/tick confirmed |
| Unbounded plugin stdout/stderr line reads | Reproduced oversized unterminated output. Limit reads to 64 KiB + 1, kill the process and record a crash on oversized stdout, and drain stderr in chunks. | CLOSED |
| memory_limit_mb presented as enforced | No cgroup, Job Object, or RLIMIT enforcement exists. The field validates a positive budget only; contract comments and security docs state that it is not OS isolation. | CLOSED: OS enforcement is Post-V4 |
| Full Windows workspace | Targeted Rust target checks passed. Added missing Linux cfg isolation in the Linux adapter. The remaining full-workspace build was blocked by absent MSVC lib.exe for bundled SQLite. | Environment limitation, not a code-level V4 blocker |
| Boundary review | Reviewed history IDs, corrupt/array JSON, interrupted SQLite migration, Windows argument quoting, IPC bounds, and request-ID cache; confirmed defects appear below. | CLOSED |

## 3. Newly Discovered Defects

| Defect and cause | Minimal fix and file | Regression evidence and V4 scope |
|---|---|---|
| Corrupt JSON lines were silently skipped, so a later write could overwrite history; appending to a legacy JSON array corrupted its format. | memory/src/store/json_file.rs: preserve load errors and block reads/writes; atomically persist complete records before updating the cache. | tests/memory.loadHistory.unit.test.rs covers corrupt-file preservation and reopening a legacy array; repairs existing persistence. |
| SQLite v1→v2 DDL and time conversion were not one transaction. | memory/src/store/sqlite.rs: migrate data and schema marker in one transaction. | A trigger rejects UPDATE: old columns, values, and version remain after failure, and migration succeeds once the fault is removed. |
| Two loops with the same millisecond and tick generated identical history primary keys. | runtime/src/adaptive_loop.rs: include process ID and per-process sequence in the ID, using one timestamp. | Same-time, same-tick loops get distinct IDs; restores history-write reliability. |
| Windows arguments with empty strings, embedded quotes, or trailing backslashes were quoted incorrectly. | service-host/src/windows/controller.rs: apply Windows command-line escaping to arguments and executable path. | Pure-function unit tests on Linux and targeted Windows check; no service installation. |
| The Linux adapter referenced Linux libc symbols when built for a Windows workspace target. | adapters/linux/src/lib.rs: crate-level Linux cfg isolation. | Linux libc errors disappeared in Windows workspace check; MSVC toolchain remained unavailable. |
| Archives and deb packages omitted the production JSON model, forcing a rules-path fallback after installation. | deploy/packaging/build_{tar,deb}.sh and deploy/linux/{install,upgrade,rollback,uninstall}.sh: package and synchronize the model plus README/LICENSE/NOTICE. | Built both packages, inspected manifests and checksums, ran the tar-staged binary in isolated Model Mode, and observed one model_loaded; installed neither package. |
| Zero benchmark ticks yielded meaningless averages; report version still said V3.2. | apps/aicore-bench/src/main.rs: reject zero ticks and use the Cargo package version. | --ticks 0 errors; NoOp/NotSupported are not presented as success. |

Previously repaired and unchanged on review: the single ModelRuntimeBinder source, shared PureRustRuntime, matching four-dimensional model consumer, binding after policy validation, neutral NoOp in SafeMode, rejected tasks not left Running, bounded IPC frames and safe Unix sockets, Ed25519 trust, and Dashboard loopback/SSE.

## 4. Modified Files

The following was the full file list for that historical engineering round, from a clean checkout to its final diff. Each change repaired an existing defect or synchronized documentation; task, policy, and runtime product semantics were unchanged.

| File | Before → after; reason |
|---|---|
| adapters/linux/src/lib.rs | Linux APIs failed on a Windows target → isolate them with Linux cfg. |
| apps/aicore-bench/src/main.rs | Zero ticks and stale version label → reject zero; report version 0.4.0. |
| contracts/src/plugin/host.rs | Ambiguous memory field → clarify that the budget is not OS enforcement. |
| deploy/linux/install.sh | Installed only binary → also install model for default-model discovery. |
| deploy/linux/upgrade.sh | Upgraded and restored only binary → back up and restore model too. |
| deploy/linux/rollback.sh | Rolled back only binary → restore matching model. |
| deploy/linux/uninstall.sh | Left installed model behind → remove that model. |
| deploy/packaging/build_deb.sh | Omitted model and legal/readme files → stage model, README, LICENSE, NOTICE. |
| deploy/packaging/build_tar.sh | Omitted model and legal/readme files → stage model, README, LICENSE, NOTICE. |
| docs/plugin/PLUGIN_SECURITY.md | Clarify absent OS enforcement and the 64 KiB line limit. |
| memory/src/store/json_file.rs | Silently skipped corruption and broke arrays on append → fail closed and replace atomically. |
| memory/src/store/sqlite.rs | Committed migration steps separately → one transaction with rollback on failure. |
| plugin/src/host/process.rs | Read unbounded stdout/stderr lines → bounded reads and crash handling. |
| runtime/src/adaptive_loop.rs | History IDs could collide → add process sequence; retain one task/tick. |
| service-host/src/windows/controller.rs | Incomplete argument escaping → correct Windows quote/backslash rules. |
| tests/memory.loadHistory.unit.test.rs | No corruption/array regressions → add two preservation tests. |
| tests/pipeline.taskPipeline.integration.test.rs | Checked only final state → assert one task completes per tick and none remain Running. |
| tests/plugin.processIsolation.integration.test.rs | No oversized unterminated line test → assert deterministic error and crash state. |
| README.md | Replace stale safety-filter and candidate status with actual test and closure status. |
| docs/CURRENT_BASELINE.md | Update old verification limits and history-ID description. |
| docs/quality/ENGINEERING_STATUS.md | Separate code closure from external certification. |
| docs/quality/RELEASE_GATE.md | Record full test evidence and remaining external gates. |
| docs/quality/VERIFICATION_RUNBOOK.md | Separate local full-suite and isolated Model Mode instructions. |
| docs/quality/V4_RELEASE_READINESS.md | Distinguish code closure from deployment certification. |
| docs/quality/V4_DEFECT_CORRECTION_REPORT.md | Replace previous NOT CLOSED status with actual evidence and limits. |

## 5. New Files

No new project source files. dist/ and target/ were generated build outputs; validation scripts and logs were under /tmp.

## 6. Deleted Files

None. Specified residual files stayed in place with identical SHA-256 hashes; tests/ retained 110 Rust files and 110 manifest entries.

## 7. Complete Test Evidence

| Command | Exit code and key result |
|---|---|
| cargo check --workspace --all-targets --locked | 0，workspace check PASS |
| cargo build --workspace --all-targets --locked | 0，workspace build PASS |
| cargo clippy --workspace --all-targets --all-features --locked -- -D warnings | 0, no warning |
| cargo test --workspace --all-targets --locked | 0; 137 test binaries, 666 passed, 0 failed/ignored/filtered; no cargo aggregate time; longest binary 30.0s |
| cargo nextest run --workspace --all-targets --locked | 0; 666 run/passed, 0 skipped; 31.223s |
| cargo fmt --all -- --check | 0 |
| Model Mode: isolated aicored --once and model_execution_unit | 0; 5/5 model unit tests; daemon single tick succeeded, model_loaded once, schema v2; staged tar binary also passed |
| bash -n for all .sh files | 0 |
| scripts/audit_v4_static.sh / scripts/audit_v32_static.sh | 0/ 0, 110 complete |
| Windows MSVC targeted service-host/adapter/IPC cargo check | 0 |
| Windows MSVC full-workspace cargo check | 1; WSL lacked lib.exe, so not PASS |

Full output was retained in /tmp/aicore-v4-final-*.log. All final Rust checks were repeated after the last Rust source edit. A later model-rollback script fix was checked with shell syntax and package rebuilding. Model Mode used AICORE_EXECUTION_MODE=model for its isolated run, not as a global override for full tests.

## 8. Local Hardware Tests

Unfiltered nextest ran and passed nine previously excluded cases: set_affinity_for_self_succeeds, executor_runs_and_reports_outcomes, full_loop_runs_and_records_metrics, history_disabled_by_default_means_no_records, history_enabled_records_tick, smoke_runs_without_panic, smoke_trace_ids_unique, smoke_mode_is_valid_enum_value, and release_does_not_panic. Results from local WSL must not be extrapolated to other CPU/GPU/NPU platforms.

## 9. Packaging

| Item | Result |
|---|---|
| Linux tar.gz archive build | PASS; contained bin, models/policy_classifier_v1.json, README, LICENSE, NOTICE, service scripts; install scripts could generate default config. |
| Debian .deb build | PASS; contained model, binary, systemd unit, maintainer scripts, README, LICENSE, NOTICE; daemon had verified built-in defaults for missing config. |
| RPM build | NOT AVAILABLE: no corresponding build script in the repository. |
| Windows package build | NOT RUN: the WSL host lacked a Windows packaging environment. |
| Checksum / content check | PASS; SHA-256 recorded in the round's output; staged binary passed Model Mode smoke. |

Built artifact SHA-256: tar.gz = 831034c2321a24cdb662ee9fadbc920961232bc84861c17e2a76d160f86f80c5; deb = 47c34edec05f5ab46304728787a85e6485d2f3b831528ed6d7bb34cef946ccd4.

No generated installer or package was installed. Maintainer, install, upgrade, rollback, and uninstall scripts were not run.

## 10. Not Executed

deb/rpm/MSI/EXE installation, Windows SCM registration, systemd installation/enabling/starting, and production deployment were NOT RUN because they were prohibited. Full native Windows runtime, vendor GPU/NPU hardware, and 24-hour stability were NOT VERIFIED. A long native benchmark was not run, so empty execution targets could not be mistaken for Model Mode or hardware certification.

## 11. Remaining Issues

### V4 Blocker

None confirmed in current code and authorized Linux verification scope.

### Non-blocking

The JSON store rewrites the full file on each append; primary production storage remains SQLite, and large-history performance can be assessed separately. IPC request-ID cache has a 4,096-entry window: IDs can be reused after they leave it. This is bounded-cache behavior, not indefinite deduplication. Full Windows workspace compilation was blocked by missing MSVC lib.exe and cannot be claimed as verified.

### Post-V4

PluginProcessSpec.memory_limit_mb validates a positive budget only; OS-level memory isolation requires separate platform work. A new ONNX engine, parallel multi-policy task batches, and new business layers are outside V4.

### Hardware Certification

Real control across other CPU/GPU/NPU devices and vendor drivers requires separate hardware. Local testing is not comprehensive certification.

### Platform Certification

Native Windows operation, SCM lifecycle, and MSVC bundled-SQLite build require verification on a suitable Windows host.

### Deployment Certification

deb/rpm/MSI installation, service lifecycle, and production stability require separate authorization and environments.

## 12. Final Decision

**V4 Engineering Closure: PASS. Can enter next stage: YES.** Within the authorized Linux/WSL engineering scope, V4 code passed compilation, Clippy, full tests, isolated Model Mode, static checks, and packaging builds. The project remained V4 0.4.0; production deployment, native Windows operation, and cross-hardware certification did not receive PASS.
