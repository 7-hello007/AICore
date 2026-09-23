# AICore V4 Final Verification Report

验证日期：2026-09-23。环境：WSL Ubuntu / x86_64-unknown-linux-gnu，rustc / cargo 1.75.0，nextest 0.9.85。版本仍为 0.4.0；没有改变 MSRV、业务目标、公开接口或引入依赖。

## 1. 最终结果

| 项目 | 结果 |
|---|---|
| V4 status | NOT CLOSED — 不宣称完整工程闭环 |
| Build | PASS：Linux workspace 全 targets check/build |
| Clippy | PASS：全 targets、全 features、-D warnings |
| Tests | 安全筛选集 PASS；未筛选完整测试 NOT RUN |
| Model Mode | PASS：隔离 daemon --once 和组合模型闭环测试 |
| Docs | 本轮发现的状态/范围/验证文档已同步；不宣称所有设计要求均已实现 |
| Residual files | PASS：7 个指定文件 SHA-256 未变；修改前文件无删除 |
| Windows | Service Host / Adapter / IPC 定向 target check PASS；完整 workspace NOT VERIFIED |

仓库开始时已有 66 个 tracked 文件修改及 3 个 untracked 文件。本报告只把本轮复现和修改归为本轮成果；原有修改保留。旧报告不能作为测试证据。

## 2. 原来存在并已经修复的问题

以下均为既有实现的缺陷修复，不增加业务能力。

| 问题与根因 | 文件 / 最小修复 | 验证 |
|---|---|---|
| Model checker 嵌套 if 触发 Clippy | `core/src/model.rs`：合并等价条件 | check、Clippy、模型测试 |
| 文件 SHA-256 快路径漏校验签名格式 | `plugin/src/signature.rs`：先校验 expected 格式，再流式 hash | 原 `verify_malformed_expected` 先失败后通过，签名 8 测试通过 |
| 被拒绝策略提前 return，已出队任务留在 Running | `runtime/src/adaptive_loop.rs`：拒绝时 fail 已消费任务，记录失败原因 | 新 regression 先得到 Running，修后 Failed；拒绝策略不会调用 executor |
| JSON 写失败仍更新缓存；append 与 retention 写盘可交错 | `memory/src/store/json_file.rs`：持锁覆盖磁盘与缓存提交，先持久化后修改缓存 | 注入临时文件路径冲突，clear/retention/profile 失败后缓存与重开数据不变 |
| 非有限输入/点积溢出产生 NaN softmax 结果 | `ai-runtime/src/pure_rust/model.rs`：检查 weights/features/logits/分母 finite | 3 个回归先失败后通过，正常 softmax 归一化及维度检查通过 |
| GPU 测试 fixture 类型为 Unknown，未进入预期分支 | `tests/runtime.binding.unit.test.rs`：使用明确 `gpu:test` 设备 ID | 保留“无 executor”断言，10 测试通过 |
| daemon 测试未强制模型模式，路径可访问共享 memory，二进制缺失时假通过 | `tests/daemon.lifecycle.integration.test.rs`：模型环境、独立 config/memory/IPC/plugins/temp、缺二进制明确失败、Windows exe 后缀、断言 tick success | 构建二进制后真实启动独立模型进程，测试通过 |
| EventBus 持 registry 读锁调用 subscriber，回调退订会死锁 | `core/src/event_bus/bus.rs`：克隆 Arc 列表后释放锁再回调 | 自退订测试先超时，修后通过 |
| Windows cfg 内把 AicoreError 直接传入 Into<String> | `service-host/src/windows/controller.rs`：显式 to_string | Windows target E0277 复现后，定向 check 通过 |
| CI lint job 缩进使 lint=null、name/steps 成为错误 job | `.github/workflows/ci.yml`：修层级；用 locked metadata 检查锁文件；Clippy 包含 all-features | YAML job 结构断言及本地命令通过；未运行远程 CI |
| 静态脚本依赖不存在的 python 别名 | `scripts/audit_v4_static.sh`：调用 python3 | 110 个 Rust 测试 manifest 完整性、Cargo/JSON 静态审计通过 |
| IPC 上限在整行分配后才检查，未换行流可无界增长 | `ipc/src/transport/{mod,linux,windows}.rs`：共享 bounded reader，最多读取上限+1 字节 | 保持 writer 打开、发送无换行超限数据，读端及时报 InvalidData |
| Unix socket bind 无条件删除已有路径 | `ipc/src/transport/linux.rs`：拒绝普通文件/符号链接/活动 socket，仅清理失效 socket | 普通文件内容保留、活动 listener 不被替换测试通过 |
| 当前状态文档仍称无工具链，路线图把 V4 描述为新增能力阶段 | README、CURRENT_BASELINE、quality、PRODUCT_EVOLUTION 等：更新为真实结果和缺陷修复范围 | 命令证据、状态交叉核对 |

`runtime/src/lib.rs` 去掉误导性的“要删除 model_binder”注释。模型组合回归加入现有测试文件，覆盖共享 runtime、Tier、Prediction 调用、Policy/Validation、CPU 路由、Binder 推理、模型 Apply/Verify、Memory、Task、Event 和 Dashboard 数据字段。原有断言没有删除、弱化或 ignore。

## 3. 检查后确认已解决/无需重复修改

- `runtime/src/model_binder.rs` 是唯一 ModelRuntimeBinder 实现；runtime/lib 导出，daemon 从 aicore_runtime 导入。
- daemon 中一个 `Arc<PureRustRuntime>` 加载一次，Provider/Binder 共享。没有用 Mock AI 替换默认路径。
- policy JSON 是 4 维，实际 RuntimeBackedProvider 与 AdaptiveLoop binder 输入均为 CPU/GPU/NPU/memory 四维；其他 ML FEATURE_DIM 不构成此消费者的维度错误。
- `AICORE_EXECUTION_MODE=model` 为实际模式变量，默认 model；模型 executor 使用虚拟 targets。GPU/NPU 的 binder 结果带 simulated 说明，不是硬件认证。
- Runtime Binding 位于 policy validation 之后；路由决策本身不执行硬件操作。
- TransparentExecutor 保留 Apply 明细；Verify 失败、Rollback 成功的 final_status 是 NoOp；SafeMode 对 NoOp 中性。
- Task deadline 仍是显式 `deadline_unix_secs`；queue 用秒比较 deadline，生命周期时间用 ms。没有机械更改公开字段。
- SQLite WAL、schema v2、ms retention 已存在；InMemory poison 返回错误。通过现有测试不等于所有损坏/掉电恢复场景已认证。
- 签名文件 hash 已使用 64 KiB 流式读取，Trusted 已要求 pinned Ed25519 key。
- Dashboard 默认 loopback，远程绑定需显式 opt-in；SSE current-state/多行 data 与现有测试保留。
- Windows 已保存 ServiceConfig、可按已保存路径 upgrade/rollback；本轮只做源码/编译/mock 检查。

## 4. 修改文件

下方“本轮文件清单”由修改前 SHA-256 快照比较生成，区分实质修复与 `cargo fmt`。不把进入任务前的修改算成本轮新增成果。

## 5. 新增文件

无新增项目源文件或文档。更新现有 V4 报告，不创建重复报告。构建产物位于 target；验证日志、runner、文件清单保存在 `/tmp/aicore-v4-*`，模型 smoke 位于独立 `/tmp/aicore-v4-model-*`。

## 6. 删除文件

None。7 个指定残留文件与修改前 SHA-256 一致；未移动、重命名或修改内容。测试源文件仍为 110 个，manifest 完整。

## 7. 测试结果和命令

完整命令/退出码清单见下方自动记录。关键结果：安全集 651 passed，0 failed；nextest 明确 9 skipped，cargo test 对应 9 filtered out。这里的 skipped 只表示本次命令筛选，没有添加 `#[ignore]`。

编译覆盖全部测试：`cargo build --workspace --all-targets --locked`。未筛选的 `cargo test --workspace --all-targets --locked` 和 nextest 全集未运行，因为以下测试会执行真实宿主控制，不能用全局 model 环境变量阻止其直接创建的真实 adapter：

1. `set_affinity_for_self_succeeds`
2. `executor_runs_and_reports_outcomes`
3. `full_loop_runs_and_records_metrics`
4. `history_disabled_by_default_means_no_records`
5. `history_enabled_records_tick`
6. `smoke_runs_without_panic`
7. `smoke_trace_ids_unique`
8. `smoke_mode_is_valid_enum_value`
9. `release_does_not_panic`

daemon 与 plugin-echo 先构建，再执行进程集成测试；避免将缺二进制的旧测试早退视为有效进程验证。稳定性测试实际默认约 30 秒，不是 1h/24h 稳定性认证。

完整 Windows workspace check 尝试退出 101：bundled SQLite 的 cc-rs 找不到 MSVC `lib.exe`。没有为此安装工具链或改项目 baseline。定向 Rust Windows check 不需要该 C 工具，已通过。

13 个 `.sh` 均通过 `bash -n`；V4/V3.2 static audit 均通过。`local_ci.sh`、`verify_project.sh` 已读取但不直接运行，因为其内部无筛选测试会违反本轮禁控要求。包装脚本只静态检查，没有构建/安装安装包，也没有执行脚本中的清理。

## 8. 未运行项目

- Real CPU/GPU/NPU/power/priority/affinity control：NOT RUN，任务禁止。
- Windows SCM / Windows Service installation：NOT RUN，任务禁止。
- systemd install/enable/start：NOT RUN，任务禁止。
- deb/rpm/msi installation：NOT RUN，任务禁止。
- 完整 Windows runtime tests、真实 NPU：NOT VERIFIED。
- 打包安装验收：NOT RUN；Shell 语法通过不是安装认证。

## 9. 剩余问题与限制

### V4 blocker / 验证门禁

- 未筛选全套测试不能在本轮安全约束下运行，故不满足用户定义的“完整 tests 通过”门禁。必须保留真实控制测试，并在另行允许的隔离硬件环境验证，或另行明确模型测试门禁的验收定义；不能伪造 PASS。
- 当前 tick 上限为 1，区别于任务提示描述的 8。当前实现避免单个 policy 错误完成多个 task；恢复批量必须逐任务保持正确策略/状态，不能只把常量改为 8。本轮未改这一调度行为。
- 本轮发现的附加待修审查项：Plugin Host stdout/stderr 仍用无界行读取；`memory_limit_mb` 只校验、不提供 OS 强制隔离。前者是现有协议边界缺陷，后者需确认既有平台范围，不能从“参数校验”推断为内存限制已生效。

### Non-blocking / 尚未全面验证

- 毫秒+tick 的历史 ID 降低重启冲突，但不能据此宣称多实例同毫秒绝无碰撞；未增加公开 ID 契约。
- JSON legacy 损坏/混合格式恢复、SQLite 迁移中断、插件全部异常输出、Windows 参数尾反斜杠/空参数语义仍需专门审查回归。
- 项目范围很大，本轮定向源码审查与测试不能被描述为所有旧清单项目逐行、全平台认证。

### Post-V4

新 ONNX 引擎、大型架构重设计、新业务模块、Agent/Intent/Planner、通用 NPU 控制等没有实现；这些属于未来工作。

### Hardware / Platform verification required

真实资源控制、服务生命周期、安装包、完整 Windows 构建/运行、长时间硬件稳定性，需要单独环境和授权。

## 10. 下一阶段判断

**Can enter next stage: NO。** 当前编译、Clippy、安全测试、模型模式证据可复核，但完整测试门禁、task batch 差异和剩余边界审查尚未闭合。保持 AICore V4，不宣布 V5，不将模型执行描述为真实硬件执行。
