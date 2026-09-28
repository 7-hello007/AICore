[英文](V4_RELEASE_READINESS.md) | [中文](V4_RELEASE_READINESS.zh-CN.md)

# AICore V4 发布就绪状态

## 当前状态

**代码工程闭环已通过；部署与广泛的平台认证仍未完成。**

## 进入下一阶段所需证据

- `cargo fmt --all -- --check`
- 在 Linux 和 Windows 上运行 `cargo build --workspace --all-targets --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- 完整执行 `aicore-tests`
- Linux 与 Windows 服务的安装、启动、状态检查、停止及卸载冒烟测试
- IPC 畸形输入、超大输入、重复请求和背压测试
- 插件信任、超时、崩溃隔离和权限测试
- 仪表盘快照、SSE 与浏览器冒烟测试
- 仅在具备相应硬件和厂商接口的机器上执行真实 GPU/NPU 能力测试
- 长期守护进程稳定性测试

`V4_DEFECT_CORRECTION_REPORT.md` 记录了 V4 代码工程门槛的证据。生产发布认证还需要在目标主机上完成服务与硬件门槛；这些外部验收不会改变既有代码门槛，也不会把项目自动改称 V5。
