# Current Engineering Baseline

本文件不是目标设计，而是当前仓库状态的入口说明。

## 当前结论

上传版本已经形成完整的 Rust workspace、Core/Contracts/Runtime/Adapter/AI/Memory/Plugin/Tier/IPC/CLI/TUI/Web 代码骨架，并且存在大量单元、契约和集成测试。

但“代码存在”不等于“工程完成”。当前仍存在：

- Main daemon 没有把多个 V3/V3.2 组件全部接入主闭环。
- GPU/NPU 真控制仍存在明确 stub。
- Windows hardware telemetry/control 仍有未实现路径。
- ONNX backend 没有真实 `ort` dependency，测试模型仍有 placeholder。
- IPC 的 Ack/handshake/security/backpressure/dedup 尚未完整实现。
- Plugin timeout/isolation/trust persistence 尚未达到生产级。
- TierHub/L0/L1/L2 已存在，但不等于已经进入 daemon 主循环。
- Web Dashboard 已有功能原型，但视觉、HTTP 实现和安全策略仍需产品化。
- 本次审计环境没有 `cargo`，所以没有把本地编译/测试结果冒充为通过；最终门禁必须在 CI / 实际 Windows/Linux 环境执行。

历史缺陷与后续路线以发布前审计记录和版本变更记录为准，不作为运行时依赖。


## Code package status
The source package is V3.2 engineering-close candidate. Target-host CI evidence remains the final gate before V4 entry.


## V3.2 修复基线

本仓库的 V3.2 代码目标是：

- Core / Runtime / Tier / Task Pipeline 形成真实闭环；
- L0 / L1 / L2 仅依赖稳定 Contracts，彼此不直接依赖；
- Windows / Linux 仅通过 Adapter 暴露平台差异；
- Terminal 与 Web Dashboard 共享同一状态数据面；
- 内存 retention 使用统一时间单位；
- IPC / Plugin / Deployment / CI 均具备生产化约束。

当前环境未安装 Rust toolchain，因此本次修复包只报告静态源码与结构性验证结果；真实平台测试必须在 Windows / Linux CI 或目标设备上执行后，才能把 V3.2 标记为最终 release。
