# Engineering Status

## Package baseline

AICore `0.4.0` has **V4 code engineering closure**. V4 corrects defects in the existing V3.2 architecture without introducing a new business layer.

Work against the broader root `destination.md` is tracked separately in
`DESTINATION_REQUIREMENTS.md` and `DESTINATION_VERIFICATION_REPORT.md`. Those
goals are not implied by V4 code closure; the current destination assessment
remains incomplete.

## Correctness invariants restored

- No runtime hardware binding is executed before policy validation.
- A dequeued task is not silently completed by another task's policy.
- Expired tasks remain auditable as `Expired`.
- GPU and NPU capability sets carry device kind and are bound separately.
- Transparent execution preserves the real apply result for observability.
- Trusted plugins require a pinned Ed25519 public key.
- Memory lock failures are surfaced rather than silently treated as success.
- Dashboard and IPC transport paths have bounded/safe handling improvements.

## External verification gate

The 2026-09-23 verification used Rust 1.75 on WSL/Linux. The unfiltered suite passed (nextest 666/666), alongside isolated model execution, Clippy and package builds. See `V4_DEFECT_CORRECTION_REPORT.md` for exact evidence. Windows runtime, service installation and broad hardware certification remain separate release gates in `V4_RELEASE_READINESS.md`.
