# V3.2 Implementation Status

## Scope

This source tree is the V3.2 engineering-close implementation candidate. The goal is to make the existing architecture internally consistent and runnable without introducing a new product version boundary.

## Implemented in source

- Unified global `docs/` architecture package is included in the source tree.
- `destination/` is excluded from the product source package.
- Core → Runtime → Adapter boundaries remain interface-driven.
- L0/L1/L2 remain peer adapters over shared contracts; no tier depends on another tier.
- Workload fusion no longer relies on filtered-list positional indexes.
- AdaptiveLoop consumes workload/task/adaptation providers through TierHub capabilities.
- Task queue removes expired entries and supports explicit cancellation/clear operations.
- JSON/SQLite memory persistence avoids the previous cache/disk ordering bug and stores SQLite record timestamps using explicit millisecond semantics.
- Local feature-weighted model is a real file-backed inference path; Mock AI is not the production default.
- AI provider and runtime routing share the same loaded local model runtime instance.
- Plugin subprocess stderr is drained to avoid pipe backpressure deadlock.
- Web dashboard is loopback-only by default, shares daemon state, has modern responsive UI, and emits SSE event ids.
- Packaging uses one workspace version source.
- Tests are flat under `tests/`.

## Platform-limited capabilities

GPU/NPU control remains capability-gated. There is no safe cross-vendor API that can be honestly claimed as universally supported on both Windows and Linux. Unsupported hardware paths return explicit `NotSupported` outcomes instead of fabricated success. Vendor-specific plugins/runtimes are the extension point.

## Release limitation

This environment has no Rust toolchain. Therefore this file does not claim that compilation, clippy, real Windows service installation, Linux service installation, or real hardware validation has passed. Those are release gates and must be evidenced by CI or target hardware.
