# Engineering Status

## Package baseline

AICore `0.3.2` is the V3.2 engineering-close implementation candidate. The product source package includes one global `docs/` architecture; versioned planning material is intentionally excluded from the runtime package.

## What is now treated as a code requirement

- Contract boundary is stable and versioned.
- L0/L1/L2 are independent providers registered through capability registries.
- AdaptiveLoop consumes workload fusion, task metadata, runtime routing, memory, transparent execution and event publication.
- Local model execution uses the checked-in JSON feature-weight model by default; placeholder ONNX assets are not part of the release path.
- Platform hardware control is capability-gated; unsupported vendor-specific controls must return an explicit unsupported result.
- Terminal and Web dashboards consume the same daemon snapshot data.
- Plugins run in a separate process host with bounded request timeouts and crash tracking.

## External verification gate

This environment has no Rust toolchain. The repository is therefore not marked release-ready until external CI/target hosts execute format, build, clippy, tests, service installation, dashboard, real workload and long-run checks.


### Verification limitation

The repository was statically repaired in this environment, but no Rust compiler/toolchain is installed here. Therefore build, clippy, Windows service, Linux service, and real GPU/NPU tests must be executed in CI or on target hosts before release promotion.
