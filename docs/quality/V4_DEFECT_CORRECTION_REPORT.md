# AICore V4 Final Engineering Closure Report

验证日期：2026-09-23。版本 0.4.0；WSL Ubuntu，Rust/Cargo 1.75.0，nextest 0.9.85。本报告记录代码工程闭环，不能替代生产部署、Windows 实机运行或跨硬件认证。

## 1. Final Status

| 项目 | 结果 |
|---|---|
| V4 Engineering Closure | PASS（代码层） |
| Build | PASS：workspace all-targets check/build |
| Clippy | PASS：workspace all-targets all-features -D warnings |
| Full cargo test | PASS：666 passed，0 failed / ignored / filtered |
| Full nextest | PASS：666 run，666 passed，0 skipped，31.223s |
| Model Mode | PASS：隔离 daemon --once、组合模型测试和归档内二进制 smoke |
| Linux local tests | PASS：包含此前未运行的 9 项本机控制测试 |
| Windows compile verification | 定向 service-host/adapter/IPC PASS；完整 workspace NOT VERIFIED，WSL 缺 MSVC lib.exe |
| Packaging build verification | tar.gz / deb PASS，内容与 checksum 已检查；未安装 |
| Docs | PASS：状态、运行手册、插件边界及 release gate 同步 |
| Residual files | PASS：7 个指定文件 SHA-256 未变，无原文件删除 |
| Can enter next stage | YES，限于代码工程条件；不代表生产发布认证 |

## 2. Previous V4 Blockers

| 上轮问题 | 当前源码核查、处置与证据 | 最终状态 |
|---|---|---|
| 完整测试未运行 | 本轮不设置全局 Model Mode，cargo test 与 nextest 均执行未筛选全集；nextest 666/666 | CLOSED |
| 1 与 8 task/tick | AdaptiveLoop 每 tick 形成一套 policy、validation、execution，当前只 dequeue 1；contracts 没有 8 的吞吐承诺。仅改常量会把不同任务错归一个执行结果。保留 1，强化逐 tick 状态断言并同步文档 | CLOSED：确认为 1 task/tick |
| Plugin stdout/stderr 无界行读取 | 未换行超限输出回归先复现失败；改为最多读取 64 KiB+1，超限 stdout 杀进程并记录 crash，stderr 分块排流 | CLOSED |
| memory_limit_mb 被误称强制限制 | 没有 cgroup/Job Object/RLIMIT 实现；字段只校验正值。contract 注释和安全文档明确为预算参数，不宣称 OS 隔离 | CLOSED：OS enforcement 为 Post-V4 |
| Windows 完整 workspace | 定向 Rust target check 通过。额外修复 Linux adapter 在 Windows target 编译时缺 cfg；随后完整 workspace 仅被 bundled SQLite 的 MSVC lib.exe 缺失阻断 | 环境限制，不是代码层 V4 blocker |
| 边界审查 | 历史 ID、JSON 损坏/数组、SQLite 中断、Windows 参数引用、IPC 上限及 request cache 已分别检查；真实缺陷见下节 | CLOSED |

## 3. Newly Discovered Defects

| 问题、根因 | 最小修复及文件 | 回归与 V4 归属 |
|---|---|---|
| JSON 损坏行被静默跳过，后续写入可覆盖用户历史；旧 JSON 数组直接追加使格式损坏 | memory/src/store/json_file.rs：加载失败保留错误并阻止读写，append 先原子持久化完整记录再更新缓存 | tests/memory.loadHistory.unit.test.rs 新增损坏文件保留及旧数组重开测试；既有持久化功能修复 |
| SQLite v1→v2 DDL/时间转换未整体事务化 | memory/src/store/sqlite.rs：迁移与 schema marker 同一事务 | 注入拒绝 UPDATE 的 trigger，失败后旧列/值/版本不变，去除故障后迁移成功 |
| 两个 loop 同毫秒同 tick 产生相同历史主键 | runtime/src/adaptive_loop.rs：ID 加进程 ID 与进程内序号，并共用一次 timestamp | 同时间同 tick 的不同 loop ID 测试；修正已有历史写入可靠性 |
| Windows 参数中的空串、内嵌引号和尾反斜杠引用错误 | service-host/src/windows/controller.rs：使用 Windows 命令行转义规则，exe 路径同样处理 | Linux 上纯函数单测和 Windows 定向 target check；未安装服务 |
| Linux adapter 在 Windows workspace target 下引用 Linux libc 符号 | adapters/linux/src/lib.rs：crate-level Linux cfg | Windows 全 workspace 的 Linux libc 错误消失；剩余 MSVC C 工具环境缺失 |
| 归档/deb 漏带生产 JSON 模型，安装后只能退回规则路径 | deploy/packaging/build_{tar,deb}.sh、deploy/linux/{install,upgrade,rollback,uninstall}.sh：打包模型，安装/升级/回滚/卸载同步该文件，打包 README/LICENSE/NOTICE | 两种包构建、清单、checksum；从 tar stage 直接在独立目录运行 Model Mode，单次 model_loaded；从未安装 |
| benchmark 零 tick 产生无意义平均值，报告版本仍为 V3.2 | apps/aicore-bench/src/main.rs：拒绝 0、用 Cargo package version | --ticks 0 返回错误；未将 NoOp/NotSupported 伪装成功 |

此前已修复且本轮复核未回退：唯一 ModelRuntimeBinder 来源、共享 PureRustRuntime、模型 4 维消费者、Policy validation 后绑定、NoOp 中性 SafeMode、任务拒绝不滞留 Running、IPC bounded frame/安全 Unix socket、Ed25519 trust、Dashboard loopback/SSE。未重复改动。

## 4. Modified Files

以下为进入本轮时 clean checkout 与最终 git diff 的完整项目文件清单。每项只修现有缺陷或同步文档；Business logic changed: NO（task/policy/runtime 产品语义保持不变）。

| 文件 | Before → After；原因 |
|---|---|
| adapters/linux/src/lib.rs | Linux API 在 Windows target 编译 → Linux cfg 隔离；跨平台编译缺陷 |
| apps/aicore-bench/src/main.rs | 0 tick 与旧版本标签 → 拒绝 0、输出 0.4.0；结果有效性 |
| contracts/src/plugin/host.rs | 内存字段含糊 → 注释明确预算非 OS enforcement；契约真实性 |
| deploy/linux/install.sh | 仅安装二进制 → 同步安装模型；默认模型可定位 |
| deploy/linux/upgrade.sh | 仅升级/回滚二进制 → 模型随升级备份/恢复；版本一致 |
| deploy/linux/rollback.sh | 仅回滚二进制 → 同步恢复模型；版本一致 |
| deploy/linux/uninstall.sh | 遗留已安装模型 → 仅清理该模型；生命周期一致 |
| deploy/packaging/build_deb.sh | 漏模型/说明和许可文件 → stage 包含模型、README、LICENSE、NOTICE；包完整 |
| deploy/packaging/build_tar.sh | 漏模型/说明和许可文件 → stage 包含模型、README、LICENSE、NOTICE；包完整 |
| docs/plugin/PLUGIN_SECURITY.md | 内存限制表述不清 → 明确无 OS enforcement 和 64 KiB 行上限 |
| memory/src/store/json_file.rs | 损坏文件静默跳过、数组追加损坏 → fail-closed 与原子替换；数据保全 |
| memory/src/store/sqlite.rs | 迁移分步提交 → 单事务；失败回滚 |
| plugin/src/host/process.rs | 无界 stdout/stderr 读取 → 有界读取和 crash 处置 |
| runtime/src/adaptive_loop.rs | 历史 ID 可碰撞 → 加进程序号；保留每 tick 一个任务 |
| service-host/src/windows/controller.rs | 参数转义不完整 → 正确 Windows 引号/反斜杠规则 |
| tests/memory.loadHistory.unit.test.rs | 无损坏/数组回归 → 增两项数据保全测试 |
| tests/pipeline.taskPipeline.integration.test.rs | 仅检查最终状态 → 每 tick 检查单任务完成/队列及无 Running |
| tests/plugin.processIsolation.integration.test.rs | 无超限无换行回归 → 增确定性错误/进程 crash 测试 |
| README.md | 旧安全筛选/候选状态 → 当前完整测试和代码闭环状态 |
| docs/CURRENT_BASELINE.md | 旧验证限制与历史 ID 描述 → 当前结果 |
| docs/quality/ENGINEERING_STATUS.md | 候选状态 → 区分代码闭环与外部认证 |
| docs/quality/RELEASE_GATE.md | 旧完整测试未运行 → 实际全套结果及外部门禁 |
| docs/quality/VERIFICATION_RUNBOOK.md | 旧 model-only 约束 → 分开描述完整本机测试与隔离模型测试 |
| docs/quality/V4_RELEASE_READINESS.md | 代码闭环与部署认证混同 → 分开判断 |
| docs/quality/V4_DEFECT_CORRECTION_REPORT.md | 上轮 NOT CLOSED 报告 → 本轮真实证据与剩余限制 |

## 5. New Files

None（项目源文件）。dist/ 与 target/ 是本轮生成的构建产物；验证脚本及日志位于 /tmp。

## 6. Deleted Files

None。指定残留文件原位保留且 SHA-256 一致；tests/ 仍为 110 个 Rust 源文件及 110 个 manifest test 条目。

## 7. Complete Test Evidence

| 执行命令 | 退出码与关键结果 |
|---|---|
| cargo check --workspace --all-targets --locked | 0，workspace check PASS |
| cargo build --workspace --all-targets --locked | 0，workspace build PASS |
| cargo clippy --workspace --all-targets --all-features --locked -- -D warnings | 0，无 warning |
| cargo test --workspace --all-targets --locked | 0，137 个 test binary 合计 666 passed、0 failed/ignored/filtered；cargo 无总时长，最长单 binary 30.0s |
| cargo nextest run --workspace --all-targets --locked | 0，666 run/666 passed/0 skipped，31.223s |
| cargo fmt --all -- --check | 0 |
| Model Mode：隔离 aicored --once 与 model_execution_unit | 0；5/5 模型单测；daemon 单 tick success，model_loaded 一次，schema v2；tar stage 二进制同样通过 |
| 所有 .sh 的 bash -n | 0 |
| scripts/audit_v4_static.sh / scripts/audit_v32_static.sh | 0 / 0，110 test manifest 完整 |
| Windows MSVC 定向 service-host/adapter/IPC cargo check | 0 |
| Windows MSVC 全 workspace cargo check | 1，WSL 无 lib.exe；不写成 PASS |

完整输出留于 /tmp/aicore-v4-final-*.log；所有最终 Rust 命令在最后一次 Rust 源码修改后重新执行。随后安装脚本的模型缺失回滚修正已通过 shell 语法及重新打包验证。Model Mode 独立设置 AICORE_EXECUTION_MODE=model，没有全局覆盖完整测试。

## 8. Local Hardware Tests

此前筛掉的九项此次均由未筛选 nextest 实际运行并 PASS：set_affinity_for_self_succeeds、executor_runs_and_reports_outcomes、full_loop_runs_and_records_metrics、history_disabled_by_default_means_no_records、history_enabled_records_tick、smoke_runs_without_panic、smoke_trace_ids_unique、smoke_mode_is_valid_enum_value、release_does_not_panic。本机 WSL 结果不外推为其他 CPU/GPU/NPU 平台认证。

## 9. Packaging

| 项目 | 结果 |
|---|---|
| Linux tar.gz archive build | PASS；含 bin、models/policy_classifier_v1.json、README、LICENSE、NOTICE、服务脚本；安装脚本按既有逻辑生成默认 config |
| Debian .deb build | PASS；含相同模型/二进制、systemd unit、maintainer scripts、README、LICENSE、NOTICE；daemon 对缺失 config 文件使用已验证的内置默认值 |
| RPM build | NOT AVAILABLE：仓库没有对应构建脚本 |
| Windows package build | NOT RUN：本机 WSL 无 Windows 打包环境 |
| checksum / 内容检查 | PASS；SHA-256 记录于本轮命令输出，stage 二进制模型 smoke 通过 |

最终构建 SHA-256：tar.gz = 831034c2321a24cdb662ee9fadbc920961232bc84861c17e2a76d160f86f80c5；deb = 47c34edec05f5ab46304728787a85e6485d2f3b831528ed6d7bb34cef946ccd4。

No generated installer/package was installed. 未运行 maintainer、install、upgrade、rollback 或 uninstall 脚本。

## 10. Not Executed

deb/rpm/MSI/EXE 安装、Windows SCM 注册、systemd 安装/启用/启动、生产部署均为 NOT RUN — intentionally prohibited。完整 Windows runtime、所有 GPU/NPU 厂商硬件和 24h 稳定性为 NOT VERIFIED；benchmark 实机长跑未执行，避免把空 targets 的真实 adapter 结果误作 Model Mode 或硬件认证。

## 11. Remaining Issues

### V4 Blocker

None confirmed in current code and authorized Linux verification scope.

### Non-blocking

JSON store 每次 append 覆写完整文件，主要生产存储仍为 SQLite；大历史数据性能可另行测量。IPC request ID cache 有 4096 条窗口上限，旧 ID 过窗后可重用，这是缓存窗口语义，非无限期去重。完整 Windows workspace cross-check 被缺少 MSVC lib.exe 阻断，不能宣称跨平台完整编译。

### Post-V4

PluginProcessSpec.memory_limit_mb 仅作正值预算校验；OS 级进程内存隔离需要单独平台实现。新 ONNX 引擎、并行多策略 task batch 和新业务层不属于 V4。

### Hardware Certification

其他 CPU/GPU/NPU 及厂商驱动的真实控制需要独立硬件；本机测试不代表全面认证。

### Platform Certification

Windows 原生运行、SCM 生命周期、MSVC bundled SQLite 构建需要目标机验证。

### Deployment Certification

deb/rpm/MSI 安装及服务生命周期、生产环境稳定性需要独立授权与环境。

## 12. Final Decision

**V4 Engineering Closure: PASS。Can enter next stage: YES。** 现有 V4 代码在授权的 Linux/WSL 工程范围内完成编译、Clippy、完整测试、隔离 Model Mode、静态与打包构建闭环。项目仍为 V4 0.4.0；生产部署、Windows 运行和跨硬件认证并未因此获得 PASS。
