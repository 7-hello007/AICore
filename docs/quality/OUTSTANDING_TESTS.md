[English](OUTSTANDING_TESTS.md) | [Chinese](OUTSTANDING_TESTS.zh-CN.md)
# Destination test status and outstanding acceptance

This is the current register for tests and acceptance checks that have not completed. The historical [verification report](DESTINATION_VERIFICATION_REPORT.md) retains the evidence from earlier runs; its older sections do not supersede this register. `PASS` applies only to the stated scope. `FAIL` records an observed incompatibility or failing execution. `SKIPPED` means deliberately not run. `NOT VERIFIED` means the required acceptance evidence is absent.

The latest completed project-level evidence predates this documentation-only closure and is stored under `env/validation/native-1790517148896/`. This round does not perform native control or change host configuration.

## Completed checks

| Check | Status | Evidence and exact scope |
|---|---|---|
| Windows workspace `cargo check --workspace --all-targets --locked --offline` | PASS | Exit 0; `check-closure.log`; Rust 1.75.0. |
| Windows workspace `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings` | PASS | Exit 0; `clippy-closure.log`. |
| Windows workspace `cargo test --workspace --all-targets --locked --offline -- --nocapture` | PASS | Exit 0; 147 target summaries, 730 passed, 0 failed, ignored or filtered; `tests-closure.log`. This was an earlier authorized native-control run, not rerun in this documentation round. |
| `cargo fmt --all -- --check` | PASS | Exit 0; `fmt-closure.log`. |
| Linux-target workspace compile and Clippy | PASS | Exit 0; `linux-check-closure.log`, `linux-clippy-closure.log`; cross-compilation only. |
| Model daemon one-shot | PASS | One model tick; `model-closure.log`; no hardware outcome claimed. |
| Windows simulated Platform Mode and named-pipe smoke | PASS | Ten ticks, five in-memory successes, zero failures; `simulator-closure.log`; simulated execution only. |
| Local Ollama `llama3.2:3b` ensemble intent through simulated daemon IPC | PASS | Typed intent result; `ollama-closure.log`; not intent accuracy certification. |
| Native DirectML GPU inference and queue probe | PASS | AMD Radeon 860M device placement; `native-gpu-2000-safe.log`; separate direct inference, not automatic daemon GPU routing. |
| Experimental QDQ NPU inference and queue probe | PASS | VitisAI node-profile evidence; `npu-qdq-final.log`, `npu-closure.log`; candidate only. |
| Owned-child process priority/affinity and exact power scheme restoration | PASS | Before/after readback and restored originals; `native-tests.log`, `native-power.log`; earlier authorized native tests. |
| Owner-only pipe anonymous-principal denial and owner connection | PASS | `ACCESS_DENIED (5)` for S-1-5-7 and owner positive; native test, not a second login user. |
| Finite native tick-call sample | PASS | 2,000 calls, 823 owned-control outcomes, p99 1.2307 ms, max 1.8938 ms, zero over a 20 ms tick-call budget; `native-gpu-2000-safe.log`; no hard real-time claim. |
| Isolated observed effect and opt-in personal-memory probe | PASS | Twelve short owned-CPU baseline/intervention pairs and consent, retention and revocation checks; `native-effect-memory.log`; not representative benefit. |
| Static audits, shell/PowerShell/Python syntax and Git whitespace | PASS | `static-closure.log`, `git-diff-check.log`. |

## Incomplete tests and reasons

| Required acceptance check | Status | Reason / next evidence |
|---|---|---|
| Production FP32 policy model NPU placement | FAIL | The installed VitisAI EP assigns the default graph entirely to CPU. CPU fallback disabled produces exit 1 as designed (`npu-inference-final.log`). The experimental QDQ graph differs in top label for 16 of 1,000 synthetic vectors; representative equivalence acceptance is needed before default promotion. |
| Automatic native daemon GPU/NPU model routing | NOT VERIFIED | Device identities are discovered, but this Windows adapter cannot read GPU/NPU utilization on this host. The corrected feature path declines inference instead of inventing zero values; the post-fix probe recorded zero accelerator bindings. A validated feature source or contract is needed. |
| End-to-end hard deadline under sustained contention | NOT VERIFIED | The finite tick-call sample excludes complete daemon persistence, IPC duration, extended load and scheduling jitter. Measure full-path distributions and misses under an agreed workload. |
| Another interactive login user's Windows named-pipe denial | NOT VERIFIED | Anonymous-principal denial and owner access passed. No second-user interactive run or credentials were available. Run the prepared client from a distinct login account without disclosing a password. |
| Production typed GPU/NPU/power plugin control | NOT VERIFIED | Mock fail-closed binding exists, but this vendor lacks a certified independent setter/readback contract connected to the production daemon. Native plugin process, denial and rollback evidence are needed. |
| Representative application effects, energy and long-term personal prediction | NOT VERIFIED | Twelve short owned-CPU pairs and one minute of read-only foreground samples are not a consented application/task corpus or long-term predictive/effect study. A registered representative task, control cohort and measured energy/throughput are required. |
| Cross-vendor and native Linux execution matrix | NOT VERIFIED | Windows hardware and Linux cross-compile evidence cannot certify other vendors/OS runtime behavior. Run capability-gated native tests on supported target machines. |
| Service lifecycle and package installation certification | SKIPPED | Installer execution is prohibited for this checkout; no service/package installation was performed. Requires a separate authorized disposable environment. |
| Generic autonomous tool-agent cooperation | SKIPPED | Tool permissions, schemas, cancellation and rollback acceptance have not been defined; existing typed intent coordination is narrower. |
| Kernel integration implementation | SKIPPED | Necessity analysis found no evidence that user-mode interfaces require a kernel component. No implementation test is applicable until a measured need and design exist. |

No native-control tests are run in this documentation round. The previous full workspace test result is retained as historical evidence, not presented as a fresh execution.
