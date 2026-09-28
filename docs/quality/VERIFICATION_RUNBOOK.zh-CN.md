[英文](VERIFICATION_RUNBOOK.md) | [中文](VERIFICATION_RUNBOOK.zh-CN.md)

# 验证运行手册

## V4 验证模式

完整本地测试集包含九项宿主机控制测试；V4 代码工程验收时曾未过滤地运行。进程测试前需构建 `aicore-daemon` 和 `plugin-echo`。模型模式应另行使用 `AICORE_EXECUTION_MODE=model` 以及供配置、状态、IPC、插件和记忆使用的全新临时目录运行；旧迁移路径还需设置 `TMPDIR`。不要为完整测试集全局设置模型模式变量。

下方 Linux 与 Windows 安装步骤描述独立的目标主机认证，不属于 V4 代码工程验收时已执行的命令。软件包仅完成构建与内容检查，未安装。

```text
cargo fmt --all -- --check
cargo build --workspace --all-targets --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-targets --locked
```

## Linux
- 构建发布版本
- 安装系统服务
- 启动守护进程并验证一个完整控制周期
- 验证 Unix 域套接字 IPC
- 验证终端和网页仪表盘使用同一快照
- 运行 L0 真实应用场景
- 运行一小时稳定性冒烟测试

## Windows
- 构建工作区
- 安装、启动和停止服务
- 验证命名管道 IPC
- 验证进程和电源控制
- 在存在 `nvidia-smi` 时验证 NVIDIA 遥测与控制
- 验证网页仪表盘

## AI
- 加载 `models/policy_classifier_v1.json`
- 验证本地推理路径
- 验证回退与超时行为
- 确认没有引用占位模型

仅凭静态源码检查不足以通过发布门槛。
