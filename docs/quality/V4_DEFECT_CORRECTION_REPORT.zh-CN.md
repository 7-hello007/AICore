[英文](V4_DEFECT_CORRECTION_REPORT.md) | [中文](V4_DEFECT_CORRECTION_REPORT.zh-CN.md)

# AICore V4 最终工程收尾报告

验证日期：2026-09-23。版本 0.4.0；WSL Ubuntu，Rust/Cargo 1.75.0，nextest 0.9.85。本报告记录代码工程闭环，不能替代生产部署、Windows 实机运行或跨硬件认证。

## 1. 最终状态

| 项目 | 结果 |
|---|---|
| V4 工程收尾 | PASS（代码层） |
| 构建 | PASS：工作区全部目标的检查与构建 |
| Clippy | PASS：工作区全部目标和特性，无警告 |
| 完整 `cargo test` | PASS：666 项通过，0 项失败、忽略或过滤 |
| 完整 `cargo nextest` | PASS：666 项运行且通过，0 项跳过，耗时 31.223 秒 |
| 模型模式 | PASS：隔离守护进程 --once、组合模型测试和归档内二进制冒烟测试 |
| Linux 本地测试 | PASS：包含此前未运行的 9 项本机控制测试 |
| Windows 编译验证 | 定向 service-host/adapter/IPC PASS；完整工作区 NOT VERIFIED，WSL 缺 MSVC lib.exe |
| 打包构建验证 | `tar.gz` 与 `.deb` 均为 PASS，已检查内容与校验和；未安装 |
| 文档 | PASS：状态、运行手册、插件边界及发布门槛同步 |
| 残留文件 | PASS：7 个指定文件 SHA-256 未变，无原文件删除 |
| 可以进入下一阶段 | YES，限于代码工程条件；不代表生产发布认证 |

## 2. 上一轮 V4 阻塞项

| 上轮问题 | 当前源码核查、处置与证据 | 最终状态 |
|---|---|---|
| 完整测试未运行 | 本轮不设置全局模型模式，cargo test 与 nextest 均执行未筛选全集；nextest 666/666 | CLOSED |
| 1 与 8 task/tick | AdaptiveLoop 每控制周期形成一套策略、校验、执行，当前只出队 1；契约没有 8 的吞吐承诺。仅改常量会把不同任务错归一个执行结果。保留 1，强化逐控制周期状态断言并同步文档 | CLOSED：确认为 1 task/tick |
| 插件 stdout/stderr 无界行读取 | 未换行超限输出回归先复现失败；改为最多读取 64 KiB+1，超限标准输出杀进程并记录崩溃，标准错误分块排流 | CLOSED |
| memory_limit_mb 被误称强制限制 | 没有 cgroup/Job Object/RLIMIT 实现；字段只校验正值。契约注释和安全文档明确为预算参数，不宣称 OS 隔离 | CLOSED：OS 强制执行为 V4 后续 |
| Windows 完整工作区 | 定向 Rust 目标检查通过。额外修复 Linux 适配器在 Windows 目标编译时缺条件编译；随后完整工作区仅被内置 SQLite 的 MSVC lib.exe 缺失阻断 | 环境限制，不是代码层 V4 阻塞项 |
| 边界审查 | 历史 ID、JSON 损坏/数组、SQLite 中断、Windows 参数引用、IPC 上限及请求缓存已分别检查；真实缺陷见下节 | CLOSED |

## 3. 新发现的缺陷

| 问题、根因 | 最小修复及文件 | 回归与 V4 归属 |
|---|---|---|
| JSON 损坏行被静默跳过，后续写入可覆盖用户历史；旧 JSON 数组直接追加使格式损坏 | memory/src/store/json_file.rs：加载失败保留错误并阻止读写，追加先原子持久化完整记录再更新缓存 | tests/memory.loadHistory.unit.test.rs 新增损坏文件保留及旧数组重开测试；既有持久化功能修复 |
| SQLite v1→v2 数据定义语句/时间转换未整体事务化 | memory/src/store/sqlite.rs：迁移与模式标记同一事务 | 注入拒绝 UPDATE 的触发器，失败后旧列/值/版本不变，去除故障后迁移成功 |
| 两个循环同毫秒同控制周期产生相同历史主键 | runtime/src/adaptive_loop.rs：ID 加进程 ID 与进程内序号，并共用一次时间戳 | 同时间同控制周期的不同循环 ID 测试；修正已有历史写入可靠性 |
| Windows 参数中的空串、内嵌引号和尾反斜杠引用错误 | service-host/src/windows/controller.rs：使用 Windows 命令行转义规则，可执行文件路径同样处理 | Linux 上纯函数单测和 Windows 定向目标检查；未安装服务 |
| Linux 适配器在 Windows 工作区目标下引用 Linux 系统库符号 | adapters/linux/src/lib.rs：软件包级 Linux 条件编译 | Windows 全工作区的 Linux 系统库错误消失；剩余 MSVC C 工具环境缺失 |
| 归档与软件包漏带生产 JSON 模型，安装后只能退回规则路径 | `deploy/packaging/build_{tar,deb}.sh`、`deploy/linux/{install,upgrade,rollback,uninstall}.sh`：打包模型，安装、升级、回滚和卸载时同步该文件，打包 README、LICENSE、NOTICE | 两种包均已构建并检查清单与校验和；从归档暂存目录直接运行模型模式，只加载一次模型；从未安装 |
| 基准测试零控制周期产生无意义平均值，报告版本仍为 V3.2 | apps/aicore-bench/src/main.rs：拒绝 0、用 Cargo 软件包版本 | --ticks 0 返回错误；未将 NoOp/NotSupported 伪装成功 |

此前已修复且本轮复核未回退：唯一 ModelRuntimeBinder 来源、共享 PureRustRuntime、模型 4 维消费者、策略校验后绑定、NoOp 中性 SafeMode、任务拒绝不滞留运行中、IPC 有界帧/安全 Unix 套接字、Ed25519 信任、仪表盘 loopback/SSE。未重复改动。

## 4. 已修改文件

以下为进入本轮时干净的检出与最终 git diff 的完整项目文件清单。每项只修现有缺陷或同步文档；业务逻辑改变: NO（task/policy/runtime 产品语义保持不变）。

| 文件 | 修改前 → 修改后；原因 |
|---|---|
| adapters/linux/src/lib.rs | Linux API 在 Windows 目标编译 → Linux 条件编译隔离；跨平台编译缺陷 |
| apps/aicore-bench/src/main.rs | 0 控制周期与旧版本标签 → 拒绝 0、输出 0.4.0；结果有效性 |
| contracts/src/plugin/host.rs | 内存字段含糊 → 注释明确预算非 OS 强制执行；契约真实性 |
| deploy/linux/install.sh | 仅安装二进制 → 同步安装模型；默认模型可定位 |
| deploy/linux/upgrade.sh | 仅升级/回滚二进制 → 模型随升级备份/恢复；版本一致 |
| deploy/linux/rollback.sh | 仅回滚二进制 → 同步恢复模型；版本一致 |
| deploy/linux/uninstall.sh | 遗留已安装模型 → 仅清理该模型；生命周期一致 |
| deploy/packaging/build_deb.sh | 漏模型/说明和许可文件 → 暂存区包含模型、README、LICENSE、NOTICE；包完整 |
| deploy/packaging/build_tar.sh | 漏模型/说明和许可文件 → 暂存区包含模型、README、LICENSE、NOTICE；包完整 |
| docs/plugin/PLUGIN_SECURITY.md | 内存限制表述不清 → 明确无 OS 强制执行和 64 KiB 行上限 |
| memory/src/store/json_file.rs | 损坏文件静默跳过、数组追加损坏 → 默认拒绝与原子替换；数据保全 |
| memory/src/store/sqlite.rs | 迁移分步提交 → 单事务；失败回滚 |
| plugin/src/host/process.rs | 无界 stdout/stderr 读取 → 有界读取和崩溃处置 |
| runtime/src/adaptive_loop.rs | 历史 ID 可碰撞 → 加进程序号；保留每控制周期一个任务 |
| service-host/src/windows/controller.rs | 参数转义不完整 → 正确 Windows 引号/反斜杠规则 |
| tests/memory.loadHistory.unit.test.rs | 无损坏/数组回归 → 增两项数据保全测试 |
| tests/pipeline.taskPipeline.integration.test.rs | 仅检查最终状态 → 每控制周期检查单任务完成/队列及无运行中 |
| tests/plugin.processIsolation.integration.test.rs | 无超限无换行回归 → 增确定性错误/进程崩溃测试 |
| README.md | 旧安全筛选/候选状态 → 当前完整测试和代码闭环状态 |
| docs/CURRENT_BASELINE.md | 旧验证限制与历史 ID 描述 → 当前结果 |
| docs/quality/ENGINEERING_STATUS.md | 候选状态 → 区分代码闭环与外部认证 |
| docs/quality/RELEASE_GATE.md | 旧完整测试未运行 → 实际全套结果及外部门禁 |
| docs/quality/VERIFICATION_RUNBOOK.md | 旧仅模型约束 → 分开描述完整本机测试与隔离模型测试 |
| docs/quality/V4_RELEASE_READINESS.md | 代码闭环与部署认证混同 → 分开判断 |
| docs/quality/V4_DEFECT_CORRECTION_REPORT.md | 上轮 NOT CLOSED 报告 → 本轮真实证据与剩余限制 |

## 5. 新增文件

无（项目源文件）。`dist/` 与 `target/` 是本轮生成的构建产物；验证脚本及日志位于 `/tmp`。

## 6. 删除文件

无。指定残留文件原位保留且 SHA-256 一致；`tests/` 仍为 110 个 Rust 源文件及 110 个清单测试条目。

## 7. 完整测试证据

| 执行命令 | 退出码与关键结果 |
|---|---|
| cargo check --workspace --all-targets --locked | 0，工作区检查 PASS |
| cargo build --workspace --all-targets --locked | 0，工作区构建 PASS |
| cargo clippy --workspace --all-targets --all-features --locked -- -D warnings | 0，无警告 |
| cargo test --workspace --all-targets --locked | 0，137 个测试二进制合计 666 项通过，0 项失败、忽略或过滤；Cargo 无总时长，最长单个二进制运行 30.0 秒 |
| cargo nextest run --workspace --all-targets --locked | 0，666 项运行且通过，0 项跳过，31.223 秒 |
| cargo fmt --all -- --check | 0 |
| 模型模式：隔离守护进程 --once 与 model_execution_unit | 0；5/5 模型单测；守护进程单控制周期成功，model_loaded 一次，模式 v2；归档暂存区二进制同样通过 |
| 所有 .sh 的 bash -n | 0 |
| scripts/audit_v4_static.sh / scripts/audit_v32_static.sh | 0 / 0，110 测试清单完整 |
| Windows MSVC 定向 service-host/adapter/IPC cargo check | 0 |
| Windows MSVC 全工作区 cargo check | 1，WSL 无 lib.exe；不写成 PASS |

完整输出留于 `/tmp/aicore-v4-final-*.log`；所有最终 Rust 命令在最后一次 Rust 源码修改后重新执行。随后安装脚本的模型缺失回滚修正已通过脚本语法及重新打包验证。模型模式独立设置 `AICORE_EXECUTION_MODE=model`，没有全局覆盖完整测试。

## 8. 本机硬件测试

此前筛掉的九项此次均由未筛选 nextest 实际运行并 PASS：set_affinity_for_self_succeeds、executor_runs_and_reports_outcomes、full_loop_runs_and_records_metrics、history_disabled_by_default_means_no_records、history_enabled_records_tick、smoke_runs_without_panic、smoke_trace_ids_unique、smoke_mode_is_valid_enum_value、release_does_not_panic。本机 WSL 结果不外推为其他 CPU/GPU/NPU 平台认证。

## 9. 打包

| 项目 | 结果 |
|---|---|
| Linux `tar.gz` 归档构建 | PASS；含二进制文件、`models/policy_classifier_v1.json`、README、LICENSE、NOTICE 和服务脚本；安装脚本按既有逻辑生成默认配置 |
| Debian `.deb` 构建 | PASS；含相同模型与二进制文件、systemd 单元、维护脚本、README、LICENSE、NOTICE；守护进程对缺失配置文件使用已验证的内置默认值 |
| RPM 构建 | NOT AVAILABLE：仓库没有对应构建脚本 |
| Windows 软件包构建 | NOT RUN：当时的 WSL 环境没有 Windows 打包环境 |
| 校验和 / 内容检查 | PASS；SHA-256 记录于本轮命令输出，暂存区二进制模型冒烟测试通过 |

最终构建 SHA-256：`tar.gz` = 831034c2321a24cdb662ee9fadbc920961232bc84861c17e2a76d160f86f80c5；`.deb` = 47c34edec05f5ab46304728787a85e6485d2f3b831528ed6d7bb34cef946ccd4。

没有安装生成的安装程序或软件包。没有运行维护、安装、升级、回滚或卸载脚本。

## 10. 未执行

`.deb`、`.rpm`、MSI、EXE 安装，Windows SCM 注册，systemd 安装、启用、启动及生产部署均为 NOT RUN，因为当时明确禁止。完整 Windows 运行时、各厂商 GPU/NPU 硬件和 24 小时稳定性为 NOT VERIFIED；未执行基准测试实机长跑，避免把真实适配器在空目标下的结果误作模型模式或硬件认证。

## 11. 剩余问题

### V4 阻塞项

无确认在当前代码与获授权的 Linux 验证范围.

### 非阻塞

JSON 存储每次追加覆写完整文件，主要生产存储仍为 SQLite；大历史数据性能可另行测量。IPC 请求 ID 缓存有 4096 条窗口上限，旧 ID 过窗后可重用，这是缓存窗口语义，非无限期去重。完整 Windows 工作区交叉检查被缺少 MSVC lib.exe 阻断，不能宣称跨平台完整编译。

### V4 后续

PluginProcessSpec.memory_limit_mb 仅作正值预算校验；OS 级进程内存隔离需要单独平台实现。新 ONNX 引擎、并行多策略任务批量和新业务层不属于 V4。

### 硬件认证

其他 CPU/GPU/NPU 及厂商驱动的真实控制需要独立硬件；本机测试不代表全面认证。

### 平台认证

Windows 原生运行、SCM 生命周期、MSVC 内置 SQLite 构建需要目标机验证。

### 部署认证

deb/rpm/MSI 安装及服务生命周期、生产环境稳定性需要独立授权与环境。

## 12. 最终决策

**V4 工程收尾: PASS。可以进入下一暂存区: YES。** 现有 V4 代码在授权的 Linux/WSL 工程范围内完成编译、Clippy、完整测试、隔离模型模式、静态与打包构建闭环。项目仍为 V4 0.4.0；生产部署、Windows 运行和跨硬件认证并未因此获得 PASS。
