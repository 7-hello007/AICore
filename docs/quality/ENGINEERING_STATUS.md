# Engineering Status

## Package baseline

AICore `0.4.0` is the **V4 defect-correction implementation candidate**. V4 corrects defects in the existing V3.2 architecture without introducing a new business layer.

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

The 2026-09-23 verification used Rust 1.75 on WSL/Linux. See `V4_DEFECT_CORRECTION_REPORT.md` for executed commands and exclusions. Safe tests and model execution are distinct from the unfiltered suite and hardware certification. V4 engineering closure is not declared; `V4_RELEASE_READINESS.md` records the remaining gates.
