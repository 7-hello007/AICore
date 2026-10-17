[English](P0_FIX_VERIFICATION_REPORT.md) | [中文](P0_FIX_VERIFICATION_REPORT.zh-CN.md)

# P0 修复与验证 — 2026-10-17

## 范围与证据

唯一项目：`C:\Users\12282\Desktop\AICore`。基线：`9002c6e4499dcbed7543f413ce2d47511e56c962`；工作分支：`codex/p0-defect-fix`。未提交 commit，未同步其他副本。

本会话前序审计登记 708 个跟踪文件，全面读取 702 个文本文件（70,240 行），检查另外 6 个二进制/生成资产的元数据和关联。初始 Git 干净，未发现适用 AGENTS.md。历史报告只作核查线索，不作本次 PASS。修复期间复核新增/修改源码、断言和调用点；未声称逐行读取二进制资产。

本次日志、命令台账、完整性清单位于 `C:\Users\12282\.codex\visualizations\2026\10\16\01a143bb-0c55-7331-af19-38acccd660f4\aicore-p0`。前序阅读覆盖位于相邻 `aicore-audit`。本报告区分组件断言、可移植夹具、模型边界、Windows 原生进程验证与物理设备认证。

10 月 17 日继续任务时保留既有 Diff，仅复核修改文件、直接依赖与保留证据，未重新全量审计。继续阶段全部由主智能体独立完成，未使用子智能体或委派。本轮最终日志位于 `closure-2026-10-17` 子目录。下文先前 RED/组件结果明确属于本次修复的前序阶段，不冒充本轮新执行 PASS。

## 逐项修复结论

| ID | 原因与修复后行为 | 主要源码证据 | 结论 / 验证边界 |
| --- | --- | --- | --- |
| C01 | L1 与出队任务可能分别提供身份、上下文和约束。现在每 tick 一致选择一个契约；L1 占用 tick 时保留 Queue 任务排队。 | `runtime/src/adaptive_loop.rs`、任务管线集成测试 | FIXED；真实队列与冲突契约回归 |
| C02 | 撤权/期限/读取错误前，或终态写入失败后仍解除清理保护。现在仅成功转换后解除；清理重新读取状态，仅失败化 Running，最多两次有界尝试，保留并发终态。 | `runtime/src/adaptive_loop.rs`、任务管线集成测试 | FIXED；超时、取消、丢弃、读写错误回归。持久存储永久不可用属于基础设施限制。 |
| C03 | 旧策略重放和部分成功冒充回滚。默认执行器在任何写入前保存原值；事务覆盖应用、读回、恢复；逐项恢复所有尝试资源，最后核对完整原值。即使恢复成功，本次失败请求仍为 AllFailed。 | `contracts/src/traits/policy_executor.rs`、`core/src/executor/{policy_executor,transaction,transparent}.rs`、`runtime/src/adaptive_loop.rs` | FIXED；精确原值、假 ACK、耦合恢复、并发与 SafeMode 故障测试 |
| C04 | Router/插件能直接调用原始 GPU/NPU executor。现在 Router 要求明确绑定 guarded actuator 和 permit；插件硬件走安全 actuator；旧原始绑定不能控制。缺少可恢复状态的原生后端写前拒绝。 | `runtime-router/src/binder.rs`、`core/src/safe_actuator.rs`、`plugin/src/activation.rs`、daemon 接线 | FIXED；真实 guard/router 配合有状态模型边界；物理控制 NOT VERIFIED |
| C05 | stdin 阻塞写入发生在超时开始前并持有进程表锁。现在每子进程工作线程/操作锁覆盖排队、完整写入、响应；其他插件独立推进；取消后 kill/reap 并 join。Windows 同时取消被继承的重叠管道操作。 | `plugin/src/host/process.rs`、`plugin/tests/process_deadlines.rs` | FIXED；11 项真实子进程回归；Unix 取消运行验证 NOT VERIFIED |
| H01 | ID 前缀/厂商展示文本成为权限依据，枚举序号成为身份。改为显式设备类型、OS 数字厂商、Windows DXGI LUID/NVIDIA UUID、Linux 规范 sysfs 身份；会话序号经 DXGI 枚举关联。 | 硬件契约、Windows/Linux 硬件遥测/能力提供者、`apps/aicored/src/runtime.rs` | FIXED；身份/伪造夹具；广泛设备矩阵 NOT VERIFIED |
| H02 | 工具/路径/设备发现被表述为控制或遥测可用。通用 GPU 能力入口与原生执行器统一声明不可恢复控制不支持；通用加速器发现不再虚构遥测；Applied 与独立 Verified 分离，模型不能生成原生 Verified。 | 平台能力/GPU 入口、NPU 遥测、硬件状态契约与 guard | FIXED；发现能力、伪造模型 Verified 回归 |
| H03 | 命令 ACK 被当成硬件生效。guard 独立保存/读取请求状态；部分写入、读回不符、过期后恢复并再次读取原值。 | `core/src/safe_actuator.rs`、硬件状态接口、simulator | FIXED；读回、部分写入、恢复错误、过期、并发故障；物理读回 NOT VERIFIED |
| H04 | 粗粒度优先级/电源档位与 auto 重置丢失原值。保存 Windows 原生 class/创建时间、Linux 精确 nice/start time、Windows GUID、Linux 自定义 profile/逐 CPU governor；进程写入/恢复绑定原身份；不可逆优先级变更拒绝。电源精确恢复显式声明，Core/插件共用资源锁。 | 进程/电源契约与适配器、策略事务、插件激活 | FIXED；可移植原值测试及安全 Windows 进程测试；原生 Linux、物理 GPU/电源恢复 NOT VERIFIED。 |

未删除、忽略或弱化已有测试。原来将旧策略重放等同恢复的断言改为更严格的原值核对。恢复成功与本次请求失败分别登记，防止 NoOp 掩盖任务失败和 SafeMode。新增 trait 默认方法保留既有实现的源码兼容；新增 `Verified` 枚举分支需要外部穷尽匹配处理。

### 源码与回归位置

以下行号对应最终工作树；完整修改文件清单与外部 Diff 证据一并保留。

| ID | 源码位置 | 实际执行的回归位置 |
| --- | --- | --- |
| C01 | [统一选择任务契约](C:/Users/12282/Desktop/AICore/runtime/src/adaptive_loop.rs:644) | [L1/Queue 冲突](C:/Users/12282/Desktop/AICore/tests/pipeline.taskPipeline.integration.test.rs:88) |
| C02 | [有界终态清理](C:/Users/12282/Desktop/AICore/runtime/src/adaptive_loop.rs:79) | [终态写入故障与并发取消](C:/Users/12282/Desktop/AICore/tests/pipeline.taskPipeline.integration.test.rs:495) |
| C03 | [原值快照](C:/Users/12282/Desktop/AICore/core/src/executor/transaction.rs:42)、[恢复及最终读回](C:/Users/12282/Desktop/AICore/core/src/executor/transaction.rs:147)、[失败分类](C:/Users/12282/Desktop/AICore/core/src/executor/transparent.rs:194) | [共享事务并发](C:/Users/12282/Desktop/AICore/tests/transparent.control.unit.test.rs:400)、[最终耦合状态核对](C:/Users/12282/Desktop/AICore/tests/transparent.control.unit.test.rs:470) |
| C04 | [路由授权](C:/Users/12282/Desktop/AICore/runtime-router/src/binder.rs:300)、[插件入口](C:/Users/12282/Desktop/AICore/plugin/src/activation.rs:506) | [Guard 身份不匹配](C:/Users/12282/Desktop/AICore/core/src/safe_actuator.rs:635)、GPU/NPU/路由集成目标 |
| C05 | [完整调用期限](C:/Users/12282/Desktop/AICore/plugin/src/host/process.rs:90) | [阻塞写入超时](C:/Users/12282/Desktop/AICore/plugin/tests/process_deadlines.rs:153)、[循环清理](C:/Users/12282/Desktop/AICore/plugin/tests/process_deadlines.rs:471) |
| H01 | [数字厂商身份依据](C:/Users/12282/Desktop/AICore/contracts/src/hardware/matrix.rs:25)、[序号映射](C:/Users/12282/Desktop/AICore/adapters/windows/src/hardware/telemetry.rs:13) | [厂商展示文本伪造](C:/Users/12282/Desktop/AICore/contracts/src/hardware/matrix.rs:108)、[拒绝 ID 前缀推断](C:/Users/12282/Desktop/AICore/contracts/src/hardware/capability.rs:161) |
| H02 | [独立 Verified 状态](C:/Users/12282/Desktop/AICore/contracts/src/traits/hardware_executor.rs:27) | [模型不能伪造原生 Verified](C:/Users/12282/Desktop/AICore/core/src/safe_actuator.rs:464)、`p0_linux_hardware` 发现/能力夹具 |
| H03 | [Guard 快照/应用/读回/恢复](C:/Users/12282/Desktop/AICore/core/src/safe_actuator.rs:34) | [独立验证](C:/Users/12282/Desktop/AICore/core/src/safe_actuator.rs:447)、[读回不符](C:/Users/12282/Desktop/AICore/core/src/safe_actuator.rs:578)、[恢复失败](C:/Users/12282/Desktop/AICore/core/src/safe_actuator.rs:607) |
| H04 | [精确电源门禁](C:/Users/12282/Desktop/AICore/plugin/src/activation.rs:587)、Windows/Linux 进程与电源控制器 | [Windows 生命周期/REALTIME 拒绝](C:/Users/12282/Desktop/AICore/adapters/windows/src/process/controller.rs:522)、[精确混合 Linux governor](C:/Users/12282/Desktop/AICore/adapters/windows/tests/linux_power_recovery.rs:59)、[拒绝粗粒度电源快照](C:/Users/12282/Desktop/AICore/tests/transparent.control.unit.test.rs:631) |

## 测试先行与组件证据

以下 Cargo 命令均使用现有 1.75 MSVC `cargo.exe`、匹配的 `rustc.exe`/PATH，并设置 `CARGO_NET_OFFLINE=true`、`--locked --offline`。环境变量仅作用于命令进程。

| 证据 / 命令 | 实际结果 |
| --- | --- |
| 修复前 `cargo test -p aicore-tests --test pipeline_task_pipeline_integration ...` | RED：原始 3 项失败；终态写入故障补测 4 项失败（`task-red*.log`、`task-finish-red.log`） |
| 任务管线、adaptive loop、queue、lifecycle 目标 | 终态写入修复后 GREEN：47 项通过、0 失败（`task-finish-green.log`）；此前 83 项选定测试明确为前一阶段证据 |
| 修复前 `cargo test -p aicore-tests --test transparent_control_unit p0_ ...` | RED：5 失败 / 1 通过；另有跨包装器交错、耦合恢复、旧粗粒度电源快照的 RED 日志。 |
| `cargo test -p aicore-tests --test transparent_control_unit --test rollback_integration ...` | 选定恢复结果及后续完整工作区结果见命令台账；最终断言包括原值恢复、最终全量读回及失败分类。 |
| Windows 进程身份与 REALTIME 可恢复性测试 | 分别捕获 RED；伪造身份测试在 RED 路径也只请求当前 mask/优先级。GREEN 包证据：先 7 项后 9 项安全原生单元测试及 16 项可移植 Linux 测试。未创建真实 REALTIME 进程。 |
| `cargo test -p aicore-plugin --test process_deadlines ...` | RED：4 通过 / 4 失败；最终 GREEN：11 通过 / 0 失败。预热后连续 12 轮清理的句柄/线程均保持 127/5。 |
| 硬件 contracts/Core/router/simulator/plugin 与 guarded 模型集成 | 独立 RED 日志覆盖绕过、发现能力、假 ACK、部分恢复、过期、模型状态、release。最新复核组：40 项库测试 + 3 项可移植 Linux 测试 PASS；前一组 guarded 路由集成 17 PASS。存在重叠，不累加成工作区总数。 |
| 首次工作区尝试 | FAIL：3 项 daemon 断言暴露可选电源不可用阻塞 CPU 控制、恢复后失败被转为 NoOp。均修复根因，未压制断言。 |
| 首轮纠正后的 daemon 复测 | 10 通过 / 1 失败，发现 runtime 尚有 NoOp 覆盖；移除该覆盖，保留失败计数。 |

可移植 Linux 测试在 Windows 编译实际可移植 Linux 模块，使用临时文件/合成进程状态，不代表原生 Linux 执行。历史命名含 `gpuReal`、`npuReal`、`realModel` 的测试以源码实际边界为准，不能据名称声称真实 LLM、ONNX、GPU/NPU 推理。

## 最终工程门槛

下列命令均于 10 月 17 日实际执行。通过测试、Check、Clippy 后，仅按 rustfmt 要求合并 `core/src/safe_actuator.rs` 中一条表达式的换行，未再改变源码语义；初始格式失败与复查通过日志均保留。

| 命令 | 最终结果 | `closure-2026-10-17` 内证据 |
| --- | --- | --- |
| `cargo test -p aicore-tests --test pipeline_task_pipeline_integration --test rollback_integration --test runtime_safe_mode_unit --test task_lifecycle_unit --test task_queue_unit --test transparent_control_unit --locked --offline -- --test-threads=1` | PASS：67 通过、0 失败、0 忽略、0 过滤；6 个目标 | `targeted-control.log`、`targeted-exits.json` |
| `cargo test -p aicore-plugin --all-targets --locked --offline -- --test-threads=1` | PASS：22 通过、0 失败（11 项库测试 + 11 项真实子进程测试）；原 8/1 包残余失败已解决 | `plugin-package.log`、`targeted-exits.json` |
| `cargo test --workspace --all-targets --locked --offline -- --test-threads=1 --skip executor_runs_and_reports_outcomes` | 已执行范围 PASS：809 通过、0 失败、0 忽略、1 过滤；150 个目标；退出 0 | `workspace-tests.log`、`workspace-test-exit.json`、`workspace-target-counts.csv` |
| `cargo check --workspace --all-targets --locked --offline` | PASS，退出 0 | `cargo-check.log` 及对应退出 JSON |
| `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings` | PASS，退出 0 | `cargo-clippy.log` 及对应退出 JSON |
| `cargo check -p aicore-adapter-linux --all-targets --target x86_64-unknown-linux-gnu --locked --offline` | PASS，退出 0；Linux 目标编译，非运行 | `linux-check.log` 及对应退出 JSON |
| `cargo clippy -p aicore-adapter-linux --all-targets --target x86_64-unknown-linux-gnu --locked --offline -- -D warnings` | PASS，退出 0；仅 Linux 目标静态检查 | `linux-clippy.log` 及对应退出 JSON |
| `cargo fmt --all -- --check` | 初始 FAIL：一处表达式换行；最终 PASS，退出 0 | `cargo-fmt.log`、`cargo-fmt-final.log` 及两次退出 JSON |
| `git -c core.safecrlf=false diff --check` | PASS，退出 0；Git 覆盖仅限本条命令 | 最终完整性台账 |

定向与工作区测试有重叠，不能累加为独立覆盖总数。Cargo 的 37 个单元/示例 harness 共 109 项通过，113 个独立集成目标共 700 项通过及 1 项安全过滤（`workspace-harness-counts.json`）。工作区测试已实际链接全部测试目标，无须重复执行 `--no-run`。809 项中包含 guarded GPU/NPU 模型集成 5/2 项、路由绑定 10 项、任务管线 20 项、SafeMode 9 项、可移植 Linux 电源/进程夹具 16 项、Linux 硬件夹具 3 项。Windows 原生进程检查使用已捕获/自建进程并执行恢复读回，不认证全局电源或物理加速器。安装/服务行为测试使用已检查的 Mock，未运行安装路径。

工作区测试明确仅过滤 `executor_runs_and_reports_outcomes`；其可选电源夹具会修改共享 OS 全局电源方案。测试原样保留。不声称完整无过滤工作区 PASS。普通测试程序与自建子进程属于授权范围；未使用安装器或服务安装。

## 环境与阻塞验证

| 环境/依赖 | 版本 / 用途 | 本机结果 / 阻塞影响 | 获取决定 |
| --- | --- | --- | --- |
| Rust/Cargo/Clippy/rustfmt | 已安装 Rust 1.75 x86_64 Windows MSVC | 直接复用；Windows 构建/测试通过现有 MSVC/SDK 链接 | 无须下载 |
| Windows C++/SDK | 现有 VS2022 MSVC 14.44、SDK 10.0.26100 | 实际 Cargo 链接使用现有工具；未安装 | 无须下载 |
| Linux 目标依赖 | `windows_x86_64_gnu 0.52.6`，离线跨目标解析选中 | 默认缓存原先缺失；继续任务发现项目 `env/cargo-home` 已有源码/压缩包。复用后离线 metadata、Linux-target Check 与 Clippy 均 PASS；Linux 原生运行仍 NOT VERIFIED。 | 无须且未下载；仅通过命令局部 CARGO_HOME 复用已有缓存 |
| ONNX/Python/LLM/模型下载 | 本次修复与模型/故障边界不需要 | 未将 ONNX 缺失当作安装理由；未声称模型推理 | 未请求 |
| 真硬件、安装包/服务矩阵 | 独立原生/产品验收 | 原生 GPU/NPU/全局电源测试 SKIPPED；Linux/跨厂商认证 NOT VERIFIED；包/服务注册 SKIPPED | 需要另行安全隔离的原生验收；未授权安装 |

## 安全与文件完整性

最终 Git 跟踪及非忽略未跟踪清单共 716 个文件：原有 708 个全部保留，新增 8 个预期文件；继续阶段文件清单无增减。规范化后的跟踪文件语义 Diff 涉及 42 个文件（最后一条表达式格式调整前为新增 4,576 行 / 删除 1,157 行）；8 个新文件包含事务实现、测试夹具、子进程测试及双语计划/报告。前序已有的大量行尾差异单独保留，继续阶段未重复转换或回退。

六个受保护文件的 SHA-256 均与最初基线完全一致；`protected-final.csv` 登记前后哈希，`hashes-after.csv`、`integrity.json` 保存清单。最终进程检查发现匹配的期限/原生测试程序及自建 sleep 子进程残留为 0（`test-processes-final.json`），本轮子进程泄漏回归也通过。未提交、切换分支、清理或修改其他副本。

继续任务开始时 `env/` 已存在（目录创建时间为 10 月 16 日 19:40:13），晚于前序 18:43 的不存在记录；其中含历史缓存和验证资产。来源为 NOT VERIFIED：单凭时间戳无法确认由谁建立或填充。已原样保留；继续阶段未创建该目录或下载资产，Linux 验证复用其中原有依赖缓存。如实记录差异，不继续声称 env 不存在。

| 受保护文件 | SHA-256（修改前 = 修改后） |
| --- | --- |
| `apps/aicore-dashboard/src/client.r` | `E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855` |
| `apps/aicored/src/runtime.rs.realPC` | `C0F2C8282D7607491DF697585BA73A6BB12B05C5F0DFC6EE6582E96E8C6B3593` |
| `apps/aicored/src/scheduler.rs.realPC` | `49FE22A93BB00F23EAB844EB3D4A80443935A50BF33DEB383902D73378A6FA16` |
| `core/src/lib.rs.realPC` | `BEC54D958926F04FE514CF82B9BE1B89CED46EBB208418BD8023DFAD5F60FD59` |
| `runtime/src/lib.rs.realPC` | `94A04FAFA04519F36FE39D58D06E1ACFB2385B6C298B1CDA06E7FD2B9ABE6D81` |
| `tests/Cargo.toml.model.should.delete` | `560DFB7DE492AD78DBD79B4BAFA0EA58154E7D6F13F19270F7B2AED4280C869C` |

没有下载、依赖/工具链/组件安装、模型 pull、安装器执行、服务注册、系统环境变量修改或其他持久系统配置变更。复用已有 Rust/缓存/MSVC。原本缺失的输出文件仍缺失，未伪造补回。target 构建产物及外部证据目录为有意保留的结果。

修复前，活动 Rust 文件 CRLF 与仓库 Unix rustfmt 规则冲突。`cargo fmt --all` 规范活动 Rust 输入，修正既有 adaptive-loop 注释缩进；未格式化受保护残留扩展名。Git 规范化后的语义差异与逐字节哈希清单分别登记。

## 剩余限制

- Linux POSIX 优先级/亲和性 API 使用数字 PID；写入前后检查保存的生命周期，仍未消除内核检查/系统调用间的竞争。Windows 使用同一 handle 完成身份验证与写入。
- 队列存储永久不可用时，进程无法保证终态持久化。清理重试有界，并始终保留并发终态。
- 共享事务锁只串行化参与协议的 AICore 控制器，不覆盖任意外部程序对 OS 资源的修改。
- 缺少精确原生可恢复状态的 GPU/NPU 内置后端拒绝控制；正向模型路径仍可用且明确 model-only。物理控制、原生读回、恢复权限及成功操作的 release 需要 provider 原生验收。
- 本次测试不证明应用效果、节能收益、硬实时认证、跨硬件覆盖或产品认证。

九项 P0 在源码/回归层均为 FIXED，适用的离线工程门槛通过。本次授权的安全测试范围内达到 P0 Engineering Closure；无过滤工作区运行和完整原生硬件/平台验收尚未完成。已执行测试没有遗留失败，也没有缺少下载阻塞已修复路径。缺少精确可恢复 provider 的物理 GPU/NPU/电源操作继续不可用；Linux 原生运行、Unix 管道取消与跨厂商认证仍需另行有界原生验收，不能报告 PASS。
