# AICore — AI Computer Control Layer

Version: 0.3.2 — V3.2 engineering-close implementation candidate

AICore 是面向 Windows / Linux 的 AI 原生计算机智能控制层。它位于应用软件与操作系统资源控制之间，通过 Context、Prediction、Policy、Validator、Executor、Runtime、Adapter 与 Memory 形成闭环。

## Current engineering state

当前源码以 V3.2 engineering-close candidate 为基线：

- Core / Contracts / Runtime / Adapter / AI Runtime / Memory / Plugin / Tier 已进入统一工程骨架。
- L0 / L1 / L2 为三个独立、可插拔的兼容等级，彼此不依赖。
- Terminal Dashboard 与 Web Dashboard 使用同一 daemon snapshot 数据面。
- 默认本地 AI 路径使用 `models/policy_classifier_v1.json` 的纯 Rust feature-weighted runtime；ONNX / vendor runtime 仍作为可插拔扩展。
- Windows / Linux 的能力均采用 capability-gated 方式；不具备平台能力时返回 `NotSupported`，不得伪造成功。

## Verification

本交付包未在当前环境执行 Rust 编译，因为环境没有 `cargo` / `rustc`。进入下一产品阶段前必须在目标 Windows/Linux CI 上执行：

```text
cargo fmt --all -- --check
cargo build --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
```

并执行实际平台 service / IPC / dashboard / GPU-NPU capability / stability smoke。

## Project structure

核心目录：

```text
contracts/     stable contracts and DTOs
core/          domain/context/policy/execution logic
runtime/       adaptive control loop
ai/            AI provider abstractions and local providers
ai-runtime/    executable local model runtimes
memory/        history and profile storage
ipc/           local IPC transport
plugin/        plugin discovery, trust and subprocess host
tier/          tier registry/capability hub
tier-l0/       zero-integration legacy adaptation
tier-l1/       AICore-aware application integration
tier-l2/       native AICore task integration
adapters/      Windows/Linux platform implementations
apps/          daemon, CLI, TUI/Web dashboard, benchmarks
tests/         flat test files
docs/          whole-project engineering documentation
models/        production-safe local model assets
deploy/        deployment and service assets
scripts/       verification and packaging scripts
```

## Documentation

完整项目设计文档位于 `docs/`，不按 V1/V2/V3/V4/V5 拆分。版本只描述能力演进；未来版本的 destination / roadmap 属于项目规划资料，不是运行时依赖，也不放入产品源码发布包。

## License

Apache License 2.0. See `LICENSE` and `NOTICE`.


## 当前工程状态

当前源码为 **AICore V3.2 工程收口实现候选**。已将 Core/Runtime/Tier/Task/Memory/IPC/Plugin/Deploy/Dashboard 等模块整理为同一主闭环，并保留 Windows/Linux capability-gated 适配。

本工作环境没有 Rust toolchain，因此此压缩包不宣称“已在真实 Windows/Linux 主机通过全部编译与硬件验收”；最终 release 必须经过项目 CI 和真实目标机验收。


## Engineering completion

See `docs/quality/ENGINEERING_COMPLETION_REPORT.md` for the source-level closure summary and the release gates that still require real Rust/Windows/Linux validation.
