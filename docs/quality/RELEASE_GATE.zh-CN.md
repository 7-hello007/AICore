[英文](RELEASE_GATE.md) | [中文](RELEASE_GATE.zh-CN.md)

# V4 发布门槛

V4 是缺陷修复与工程收尾阶段。进入下一阶段需要当前范围的证据；模型模式验证不能描述为硬件认证。

| 门槛 | 要求 | 证据 |
|---|---|---|
| 源码 | 工作区可构建 | `cargo build --workspace --all-targets --locked` |
| 静态检查 | Clippy 无警告 | `cargo clippy --workspace --all-targets --locked -- -D warnings` |
| 测试 | 所有测试通过 | `cargo nextest run --workspace` 或 `cargo test --workspace` |
| Linux | 守护进程与服务冒烟测试 | Linux 持续集成目标 |
| Windows | 服务与 IPC 冒烟测试 | Windows 持续集成目标 |
| AI | 本地模型可加载 | 运行时集成测试 |
| 仪表盘 | 终端与网页显示一致 | 仪表盘集成与端到端测试 |
| 插件 | 隔离、超时与签名检查 | 插件集成测试 |
| L0 | 无需应用改造的场景通过 | L0 真实应用可执行文件 |
| 长时间运行 | 在规定时长内保持稳定 | 夜间测试证据 |

当时的证据记录在 `V4_DEFECT_CORRECTION_REPORT.md`。未过滤的 Linux 测试通过了 666 项；隔离的模型模式另行验证。表中还列有更广泛的部署与平台认证门槛：Windows 运行时、服务安装和软件包安装未在该次任务中执行，不能算作代码工程闭环的证据。
