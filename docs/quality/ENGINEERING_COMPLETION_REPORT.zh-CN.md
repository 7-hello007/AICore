[英文](ENGINEERING_COMPLETION_REPORT.md) | [中文](ENGINEERING_COMPLETION_REPORT.zh-CN.md)
# AICore V3.2 工程完成报告

> 历史 V3.2 记录。当前基线为 V4；参见 `V4_DEFECT_CORRECTION_REPORT.md` 和 `V4_RELEASE_READINESS.md`。

## 1. 软件包标识

- 工作区版本：`0.3.2`
- 产品阶段：**V3.2 工程收尾实现候选版**
- 产品文档：仅使用通用 `docs/`
- `destination/`：有意排除在产品源码包之外

## 2. 此版本在源码层解决的问题

| 范围 | 源码层处理结果 |
|---|---|
| 工作负载融合 | 数据来源与观测值的配对不再依赖过滤后的索引 |
| 自适应循环 | 由 TierHub 路由的工作负载、任务与适应提供者已进入主循环 |
| 任务队列 | 删除过期任务；支持批量出队与队列清理 |
| AI 运行时 | 真实本地 JSON 模型为生产基线；提供者和路由共享已加载的运行时 |
| 记忆 | 修复 JSON 缓存写入顺序；保留 SQLite WAL；记录时间戳列采用明确的毫秒语义并支持旧版迁移 |
| 时间模型 | 对外事件、任务和执行时间戳使用明确的毫秒字段；任务 API 指定的截止时间字段仍保持秒单位契约 |
| IPC 与仪表盘 | 仪表盘读取守护进程的单一数据平面；网页事件流发布事件标识和可安全重连的当前快照 |
| 网页仪表盘 | 默认绑定本地地址，提供自适应界面、实时状态以及运行时、上下文和资源面板 |
| 插件进程 | 持续读取标准错误输出，超时终止子进程，并跟踪进程级宿主状态 |
| Windows 服务 | 服务操作使用已安装配置；升级和回滚不再依赖硬编码的二进制路径 |
| 打包 | 工作区版本是唯一的打包版本来源 |
| 仓库质量 | 平铺的测试、锁定的依赖、许可证，以及 JSON、TOML 和脚本静态检查 |

## 3. 有意保留的能力边界

GPU/NPU 控制以能力探测为前提。不存在能诚实宣称适用于所有 Windows 和 Linux 机器的通用跨厂商控制 API。不支持的路径必须返回 `NotSupported`，不能伪装成功。厂商专用实现仍位于现有适配器和插件契约之后。

## 4. 验证状态

当时的环境完成了仓库静态检查：

- `tests/Cargo.toml` 引用了 109 个测试源文件，所有路径均存在。
- `tests/` 没有嵌套目录。
- Python 的 TOML 解析器成功解析所有 `Cargo.toml` 文件。
- 所有仓库 JSON 文件均成功解析。
- `Cargo.lock` 存在。
- `LICENSE` 存在。
- 产品包中不含 `destination/`。
- 不存在模型 `.placeholder` 残留。
- 命令行脚本通过 `bash -n`。
- `scripts/audit_v32_static.sh` 通过。

## 5. 仍需目标主机证据的发布门槛

当时的执行环境没有 Rust 工具链，也无法提供 Windows 或 Linux 的真实硬件与服务证据。因此，以下项目**在本报告中不宣称通过**：

1. `cargo build --workspace --all-targets`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo nextest run --workspace` 或 `cargo test --workspace`
4. Windows 服务安装、启动、停止、升级和回滚
5. Linux 服务或软件包安装
6. 目标硬件上的真实 GPU/NPU 遥测与控制
7. Windows/Linux 运行 24 小时的夜间稳定性测试
8. 安装程序或软件包的最终冒烟测试

只有在持续集成或目标主机提供这些门槛的证据后，才能推进到下一个产品阶段。
