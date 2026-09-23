# AICore — AI Computer Control Layer

Version: 0.4.0 — V4 defect-correction engineering closure

AICore 是面向 Windows / Linux 的 AI 原生计算机智能控制层。它位于应用软件与操作系统资源控制之间，通过 Context、Prediction、Policy、Validator、Executor、Runtime、Adapter 与 Memory 形成闭环。

## Current engineering state

当前源码以 V4 defect-correction engineering closure 为基线：

- Core / Contracts / Runtime / Adapter / AI Runtime / Memory / Plugin / Tier 已进入统一工程骨架。
- L0 / L1 / L2 为三个独立、可插拔的兼容等级，彼此不依赖。
- Terminal Dashboard 与 Web Dashboard 使用同一 daemon snapshot 数据面。
- 默认本地 AI 路径使用 `models/policy_classifier_v1.json` 的纯 Rust feature-weighted runtime；ONNX / vendor runtime 仍作为可插拔扩展。
- Windows / Linux 的能力均采用 capability-gated 方式；不具备平台能力时返回 `NotSupported`，不得伪造成功。

## Verification

当前已在 WSL/Linux、Rust 1.75 执行完整未筛选测试（nextest 666/666）、编译、Clippy、隔离 Model Mode 和打包构建验证。代码层 V4 工程闭环及外部平台认证边界见 [V4 验证报告](docs/quality/V4_DEFECT_CORRECTION_REPORT.md)。

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-targets --locked
cargo nextest run --workspace --all-targets --locked
```

本轮仅允许 `AICORE_EXECUTION_MODE=model`。测试中真实 affinity / power / GPU 控制项不得在本轮执行；真实 service 和安装验收属于另行授权的平台验证。

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

当前源码为 **AICore V4 缺陷修复工程闭环**。Core/Runtime/Tier/Task/Memory/IPC/Plugin/Deploy/Dashboard 保持既有主闭环和 Windows/Linux capability-gated 适配。

本轮验证包含现有本机控制测试，但不包含服务安装或全硬件认证。Windows 定向交叉编译通过不等于 Windows 完整运行验证。


## Engineering completion

See `docs/quality/V4_DEFECT_CORRECTION_REPORT.md` for current V4 engineering closure evidence and `docs/quality/V4_RELEASE_READINESS.md` for deployment and platform certification gates. The V3.2 completion report remains historical.
