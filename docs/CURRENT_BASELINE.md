# Current Engineering Baseline

## 当前结论

当前源码版本为 **AICore V4 defect-correction engineering closure (`0.4.0`)**。V4 的边界是修复 V3.2 已有实现中的缺陷，不增加新的产品业务，不删除既有产品逻辑。

本轮重点修复：

- Runtime route decision 与有副作用的 binding 分离；binding 只能在 Policy Validator 接受后执行。
- TaskContract 已有 priority / power / GPU / NPU / privacy / task_id 约束进入现有 policy 链；每 tick 不再把一个 policy 错误完成多个 task。
- 过期 task 保留并进入 `Expired` 状态。
- Platform Runtime Binder 注入实际探测到的 GPU/NPU capability，并区分 GPU/NPU 设备类型。
- TransparentExecutor 保留真实 Apply ExecutionResult；rollback/verify 后 final status 不再伪装为 Apply 成功。
- Model verify 覆盖 process priority / affinity / background throttle。
- Historical record id 包含时间戳、进程 ID 和进程内序号，避免同毫秒多 loop 的重复。
- Trusted plugin 强制 pinned Ed25519 public key；补齐既有 permission model 中缺失权限。
- IPC 增加消息上限，修复 Windows named-pipe 首实例生命周期竞态，并限制 connection 内 dedup 集合增长。
- Memory lock poison 不再静默成功；JSON profile 改为临时文件 + rename。
- Dashboard HTTP 请求头读取不再固定一次 8192 字节，增加安全响应头，SSE 支持多行 data。
- Workspace 版本、MSRV 元数据和 release profile 更新为 V4。

## 验证边界

2026-09-23 已使用 WSL/Linux Rust/Cargo 1.75 验证当前源码。编译、Clippy、未筛选完整测试（nextest 666/666）、隔离模型闭环及 package 构建已通过；详见 `quality/V4_DEFECT_CORRECTION_REPORT.md`。本轮允许并运行了既有本机控制测试，V4 代码工程闭环已完成；安装与跨硬件/Windows 运行认证仍独立进行。

V4 release gate 必须在目标环境运行：

```text
cargo fmt --all -- --check
cargo build --workspace --all-targets --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-targets --locked --no-run
```

当前 AdaptiveLoop 每 tick 消费一个任务；每个 tick 只形成一个 policy / validation / execution 结果，不能宣称支持 8 个独立任务/tick。完整本机测试与隔离 Model Mode 是两种不同证据；本轮没有安装服务或安装包。
