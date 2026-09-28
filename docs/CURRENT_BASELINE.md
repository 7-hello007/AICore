[English](CURRENT_BASELINE.md) | [Chinese](CURRENT_BASELINE.zh-CN.md)
# Current engineering baseline

The workspace version is AICore `0.4.0` on Rust 1.75. V4 defect correction achieved a code engineering gate for the earlier architecture; subsequent work against the broader [destination](../destination.md) added code, simulation and bounded native probes. Passing a code gate does not mean the destination product has completed platform acceptance.

## Implemented boundaries

- Contracts distinguish successful application, unsupported capability, skipped action, failure and NoOp. The validator precedes state-changing binding; readback, rollback and SafeMode preserve failure semantics.
- Continuous Model Mode and Platform Mode use separate slow preparation and fast consumption paths. The platform simulator supplies captured fixtures and in-memory actuation. Native adapters discover available capabilities instead of inventing unavailable readings.
- L0/L1/L2 task paths, application identity checks, version-negotiated IPC, pinned plugin trust and typed fail-closed capability binding are present at their documented scopes.
- Opt-in personal memory, retention, observed feedback, local JSON/ONNX inference and configurable Ollama intent providers are present. Model and simulated results remain distinct from native device effects.
- The current desktop verification record includes workspace compilation, Clippy, formatting, an earlier unfiltered 730-test run, model/simulator smoke and explicitly scoped native probes. This documentation round reruns only non-control checks.

The [verification report](quality/DESTINATION_VERIFICATION_REPORT.md) contains completed evidence and restoration records. The separate [outstanding tests](quality/OUTSTANDING_TESTS.md) file is the current register for incomplete acceptance and its reasons. Historical V3.2/V4 reports remain dated evidence, not a replacement for the current register.
