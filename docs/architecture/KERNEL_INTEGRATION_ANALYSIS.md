[English](KERNEL_INTEGRATION_ANALYSIS.md) | [Chinese](KERNEL_INTEGRATION_ANALYSIS.zh-CN.md)
# Kernel integration necessity analysis

Date: 2026-09-28. Scope: analysis only. No driver, kernel hook, service,
registry, boot setting or installer was introduced. The root destination
makes kernel integration conditional on measured need.

## Evidence

The Windows native probe used PlatformTickDriver, real telemetry, an owned
child and controller readback. Earlier GPU-route results are **withdrawn**:
the pre-fix feature builder substituted zero for missing GPU/NPU utilization.
After correction, 2000 tick calls had 823 owned-control outcomes, p99 1.2307 ms,
maximum 1.8938 ms and zero misses against a 20 ms tick-call budget. GPU model
binding correctly declined because device utilization was unavailable. A
separate DirectML queue executed the policy model on GPU; an explicitly
selected experimental QDQ candidate executed every profiled node on the NPU
(about 680 ms). Its 16/1000 synthetic top-label discrepancies prevent
production-equivalence acceptance.

These samples exclude the harness sleep and do not measure complete daemon
state/event/IPC write latency, sustained contention or thermal load. They do
not establish worst-case or hard deadlines. Native priority, affinity and exact
power GUID APIs worked in user mode with original-state restoration.

## Decision

No measured failure currently requires kernel code. Keep platform operations
behind adapters and continue bounded slow-side preparation, immutable handoff,
expiry and SafeMode. Missing vendor utilization, production NPU-model accuracy,
second-login-user ACL coverage and representative application effect data do
not become solvable merely by placing Core in a driver.

Windows scheduling is preemptive and affected by competing runnable threads,
interrupts and other workloads. A larger load/thermal corpus and an end-to-end
latency budget are needed before any hard-deadline claim. If a measured accepted
requirement later cannot be met through user-mode APIs, document that exact
gap, privilege/signing/deployment costs, recovery and platform matrix before
making a separate kernel decision. This analysis does not implement a driver.

Generic autonomous tool-agent execution remains a separate design decision:
tool schemas, per-tool identity/authorization, cancellation, side-effect
rollback and evaluation are not specified by the live intent contract. Existing
local intent consensus coordinates bounded classification proposals; it is not
arbitrary tool collaboration.

Sources: [Microsoft scheduling](https://learn.microsoft.com/en-us/windows/win32/procthread/scheduling),
[DirectML EP](https://onnxruntime.ai/docs/execution-providers/DirectML-ExecutionProvider.html),
[AMD WinML EP](https://ryzenai.docs.amd.com/projects/WinML/en/latest/winml_ep.html).
Local evidence: `env/validation/native-1790517148896/native-gpu-2000-safe.log`,
`native-npu-queue.log` and `native-power.log`.
