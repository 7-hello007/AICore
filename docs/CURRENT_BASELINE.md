# Current Engineering Baseline

## 当前结论

当前源码版本为 **AICore V4 defect-correction implementation candidate (`0.4.0`)**。V4 的边界是修复 V3.2 已有实现中的缺陷，不增加新的产品业务，不删除既有产品逻辑。

本轮重点修复：

- Runtime route decision 与有副作用的 binding 分离；binding 只能在 Policy Validator 接受后执行。
- TaskContract 已有 priority / power / GPU / NPU / privacy / task_id 约束进入现有 policy 链；每 tick 不再把一个 policy 错误完成多个 task。
- 过期 task 保留并进入 `Expired` 状态。
- Platform Runtime Binder 注入实际探测到的 GPU/NPU capability，并区分 GPU/NPU 设备类型。
- TransparentExecutor 保留真实 Apply ExecutionResult；rollback/verify 后 final status 不再伪装为 Apply 成功。
- Model verify 覆盖 process priority / affinity / background throttle。
- Historical record id 加入时间戳，避免 daemon 重启后重复。
- Trusted plugin 强制 pinned Ed25519 public key；补齐既有 permission model 中缺失权限。
- IPC 增加消息上限，修复 Windows named-pipe 首实例生命周期竞态，并限制 connection 内 dedup 集合增长。
- Memory lock poison 不再静默成功；JSON profile 改为临时文件 + rename。
- Dashboard HTTP 请求头读取不再固定一次 8192 字节，增加安全响应头，SSE 支持多行 data。
- Workspace 版本、MSRV 元数据和 release profile 更新为 V4。

## 验证边界

2026-09-23 已使用 WSL/Linux Rust/Cargo 1.75 验证当前源码。编译、Clippy、模型闭环和经过安全筛选的测试已执行；详见 `quality/V4_DEFECT_CORRECTION_REPORT.md`。未筛选全套测试包含真实硬件控制，按本轮约束未运行，V4 closure 尚未宣布。

V4 release gate 必须在目标环境运行：

```text
cargo fmt --all -- --check
cargo build --workspace --all-targets --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-targets --locked --no-run
```

本轮只执行安全筛选测试和 Model Mode；服务安装、真实硬件控制不属于本轮可执行验证。当前 AdaptiveLoop 每 tick 消费一个任务，不能宣称已经支持 8 个独立任务/tick。
