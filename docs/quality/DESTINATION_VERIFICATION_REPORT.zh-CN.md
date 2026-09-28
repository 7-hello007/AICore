[英文](DESTINATION_VERIFICATION_REPORT.md) | [中文](DESTINATION_VERIFICATION_REPORT.zh-CN.md)

# AICore 目标设计实现与验证报告

## 续作：2026-09-28（受控目标与加速器会话）

**结论：代码验证 PASS；实验性 GPU 与 NPU 原生执行 PASS；目标产品验收 NOT VERIFIED。**
该轮只修改了桌面副本 `C:\Users\12282\Desktop\AICore_V4_Windows`，未同步 WSL。
当时已获准运行原生测试，且未过滤本机控制测试；仍禁止执行安装包。
下载内容与生成的证据均保存在项目的 `env/` 下。
以下各节记录历史轮次，不代表当前的测试授权或执行状态。

### 已完成的代码与原生证据

- Windows 电源接口修复了平衡方案标识字节、原生标识处理、真实接口返回码映射、资源释放和规范化标识校验；操作前后还会精确记录与恢复原始方案。自定义或未知的方案标识也能恢复，不必假装属于标准 `PowerMode`。
- 统一的 Windows 硬件适配器现在能发现真实 DXGI 硬件身份和存在的 SetupAPI 计算加速器，并排除软件 GPU。缺失的利用率、温度和控制能力仍明确为不可用；发现身份不等于取得可用遥测或执行权限。
- 显式的 ONNX 桥支持 CPU、DirectML 和 VitisAI 执行提供者，可选择预装的执行提供者库，要求明确设备序号，校验模型模式，限制子进程，并禁用加速器会话的 CPU 回退。节点提供者分析证明实际放置。NPU 缓存和临时文件留在项目 `env/` 下；不调用应用商店或执行提供者的自动安装接口。
- 仅管理推理会话的 `QueuedModelBinder` 把既有加速器准入、能力、过期及分派租约接到真实 ONNX 推理。Windows 守护进程可通过配置注入该组件；可能改变状态的绑定仍须经过策略校验、过期和安全模式检查。多个 NPU 的身份无法明确映射时默认拒绝。原生 GPU/NPU 队列探针在显式输入上完成真实设备推理；真实的 `PlatformTickDriver` 探针还控制了受控子进程。其强制 GPU 路由在 GPU/NPU 利用率不可用时正确**拒绝**模型推理。因此，直接队列放置已验证，但原生遥测驱动的自动守护进程路由仍为 **NOT VERIFIED**。
- DirectML 策略模型在 AMD Radeon 860M、DXGI 序号 0 上的推理为 PASS。策略 ONNX 资产从既有 JSON 权重导出，具有 4 个特征和 5 个标签；它不是训练过的生成式语言模型，也没有修改全局 GPU 时钟或电源设置。
- 现有 AMD WinML NPU 执行提供者能够注册并发现真实 NPU。默认 FP32 策略图全部分配到 CPU；关闭回退后按设计拒绝执行。由相同权重生成的实验性 QDQ 候选模型在 VitisAI NPU 上运行了**所有已分析节点**，并得到真实队列响应。它与 CPU 上 FP32 模型的 1,000 个确定性合成样本比较中，最大绝对分数差为 0.00354925，最高分标签有 16 次不同，因此不能视为生产等效替代。生产默认值仍为 FP32，其 NPU 验收为 **NOT VERIFIED**。初次失败会话的清理曾发生进程崩溃；清理异常追踪和设备引用后修复该情况。会话创建后的库注销在 Python 退出期间也可能触发厂商库崩溃，因此单请求子进程保留进程级注册并随进程退出。两项负例和一项正例回归均正常退出。没有调用全局 NPU 设置器。
- 原有进程与流水线测试继续启用。受控子进程样本先记录状态和拟改值，再修改、读回并恢复优先级与亲和性，最后只终止自己的子进程。无效 PID 测试现在要求真实错误，避免恒真断言。原先只假定 NVIDIA 的设备身份测试也已改为按真实 DXGI 发现结果判断。
- 仅允许所有者的原生命名管道拒绝模拟匿名主体 SID S-1-5-7，返回访问被拒绝错误码 5，随后接受所有者连接。调用 `RevertToSelf` 后再次读取线程令牌，确认不存在令牌（错误码 1008）。没有持久账户或访问控制变更。这**不能**认证另一位实际登录用户的访问拒绝。
- 已发现 GPU/NPU 的缺失利用率不再被 `AdaptiveLoop` 的模型特征链路填成零。不存在的硬件可以提供真实零值；存在但无法测量的硬件则触发推理降级。缺少推理载荷时，`ModelRuntimeBinder` 返回 `executed=false`，防止虚假的成功绑定结果。
- 采集了 12 对受控 CPU 工作负载的真实基线与干预样本。时间戳、CPU 与内存观测及应用身份进入隔离的、需主动同意的 SQLite 记忆存储；默认拒绝、保留期、撤销同意和按逻辑清除历史均经检查。这些短时样本不是代表性应用收益、长期预测、节能或法证级安全删除的证据。另一次经同意的 60 秒只读原生观测通过 Windows 适配器记录了一个真实前台进程名称，不含窗口标题，并保留 CPU、内存与明确不可用的 GPU/NPU 利用率。由于契约要求显式注册任务，没有将这一个进程的一分钟观测写入个人数据库；它也不是代表性个人画像。
- 内核集成必要性分析见 `docs/architecture/KERNEL_INTEGRATION_ANALYSIS.md`，目前没有测量证据证明必须引入内核组件。泛化自主工具代理仍需单独设计工具权限、模式与回滚语义；现有本地意图共识不等于任意工具协作。
### 项目一级最后命令和结果

所有 Cargo 命令使用既有的 Rust **1.75.0**、项目内 `env/cargo-home`，
以及 `--locked --offline`；未更改最低支持的 Rust 版本。日志目录为：
`env/validation/native-1790517148896/`.

| 执行的命令/检查 | 状况/退出/证据 |
|---|---|
| `cargo check --workspace --all-targets --locked --offline` | PASS，退出码 0; `check-closure.log` |
| `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings` | PASS，退出码 0; `clippy-closure.log` |
| `cargo test --workspace --all-targets --locked --offline -- --nocapture` | PASS，退出码 0；147 个测试目标，**730 通过、0 失败、0 忽略、0 过滤**；`tests-closure.log` |
| `cargo fmt --all -- --check` | PASS，退出码 0；`fmt-closure.log`；`model_binder.rs` 最初的换行格式问题已通过 `cargo fmt --all` 修正 |
| Linux 目标工作空间检查和 Clippy，另加 `--target x86_64-unknown-linux-gnu` | PASS，均退出码 0；`linux-check-closure.log`、`linux-clippy-closure.log`；使用项目内既有的 Zig 编译器。这只是编译与静态证据，并非 Linux 原生运行。 |
| `python -I -S scripts/models/test_onnx_bridge.py`，显式指定项目内包和已有 NPU DLL | PASS，退出码 0；四项原生回归覆盖真实 CPU、真实 DirectML、重复拒绝默认 FP32 NPU 请求，以及实验性 NPU 执行；`onnx-bridge-final.log` |
| 直接请求默认 FP32 模型在 NPU 上推理 | FAIL，兼容性不满足，退出码 1；`npu-inference-final.log`；这是预期的默认拒绝行为。 |
| 直接请求实验性 QDQ 模型在 NPU 上推理 | PASS，退出码 0；所有节点位于 VitisAI；`npu-qdq-final.log`；候选模型精度未获生产认可。 |
| `native_npu_queue_probe.exe`，显式指定 QDQ 候选模型 | PASS，退出码 0；得到真实 PnP NPU 队列响应并释放子进程会话；`npu-closure.log` |
| `quantize_policy_npu.py`，使用既有项目内包 | PASS，退出码 0；候选模型的 SHA-256 可重复，完成 1,000 个合成向量对比；`quantization-rebuilt.log` |
| `native_gpu_queue_probe.exe`，修正未知遥测语义后 | PASS，退出码 0；独立的真实 DirectML 队列推理，以及 **2,000** 次真实调用和 823 个受控进程控制结果。强制 GPU 路由因缺少遥测而正确拒绝执行（0 次 GPU 绑定）；`native-gpu-2000-safe.log` |
| 较早的 2,000 次 GPU 探针 | 仅为历史证据；第 99 百分位 0.3106 毫秒，最大值 1.1764 毫秒，但修正前把未知利用率当作零，因此其 38 次 GPU 路由不再作为当前集成证据；`native-gpu-2000.log` |
| `native_observation_probe.exe`，用户明确同意 | PASS；60 个间隔一秒的只读样本，涉及一个前台进程；全部 60 个样本的 GPU/NPU 利用率均不可用；`native-observation.log` |
| `cross_user_acl_server.ps1 -Seconds 30`，所有者自检 | PASS；所有者成功连接，受控测试守护进程退出；`cross-user-server-selftest.log`；另一登录用户的客户端仍为 NOT VERIFIED。 |
| 原生电源方案克隆包装脚本 `scripts/verify/native_power_snapshot.ps1` | PASS；原始 GUID 设置、读回、恢复，且最终方案列表与原始列表一致；`native-power.log` |
| 受控 CPU 效果与记忆探针 `owned_process_probe` | PASS，退出码 0；12 对实测数据，隔离验证同意、保留与撤销；`native-effect-memory.log` |
| `aicored --once`、`AICORE_EXECUTION_MODE=model`、隔离的配置、状态、数据库、插件与管道 | PASS，退出码 0；完成一次模型调用；`model-closure.log`；仅为模型执行。 |
| `scripts/verify/simulated_platform_windows_smoke.ps1` | PASS；10 次调用、5 次内存模拟成功、0 次失败，并查询本地管道；`simulator-closure.log`；仅为模拟控制。 |
| `simulated_platform_ollama_intent.ps1 -Backend ensemble` | PASS；本机 `llama3.2:3b` 与 JSON 提案经命令行和模拟守护进程 IPC 达成共识，组合置信度为 0.97611916；`ollama-closure.log` |
| 源脚本语法、PowerShell 解析器、Python 语法树、`audit_v32_static.sh`、`audit_v4_static.sh` | PASS，退出码 0；核对 117 个已声明的 Rust 测试文件，并解析 12 个 PowerShell 与 5 个 Python 脚本；`static-closure.log` |
| `git -c core.safecrlf=false diff --check` | PASS，退出码 0；`git-diff-check.log`；无空白字符错误。 |

`AICORE_SMOKE_SECONDS=1` 只限定既有冒烟测试的时长；未使用跳过或忽略标记。
首次完整测试因过时的“仅 NVIDIA”断言失败；按平台实际身份修正断言后，后续完整测试通过。
没有把断言改为恒真，也没有删除测试。

### 原生测量和范围

修正后的原生快侧测量包含 **2,000 次调用**、823 个经验证的受控进程控制结果。
由于 GPU/NPU 利用率不可用，**GPU 绑定执行为 0 次**。第 99 百分位为 **1.2307 毫秒**，
最大值为 **1.8938 毫秒**，20 毫秒调用预算的超时次数为零。
另一次修正后的 1,000 次调用有 415 个受控结果，第 99 百分位 1.0431 毫秒、最大值 2.3898 毫秒。
独立的真实 DirectML 队列推理通过。较早的 2,000 次测量有 38 次 GPU 绑定，
但当时把未知利用率填成零，故这些结果**已撤回**，不作为当前完整路由证据。
原生期限测量覆盖真实受控进程操作，不覆盖完整的自动加速器路由。

测试框架休眠、完整守护进程 IPC 与状态持久化、长期资源争用和热负载不在这次测量内。
**硬实时期限仍为 NOT VERIFIED**。受控 CPU 样本略为正向的评估不能代表产品性能；
未知的能耗和吞吐数据仍为空值。生产效果反馈入口拒收模拟层证据。

### 本地业务和恢复

每次操作受控目标前均打印原值、拟改值和回滚值。
测试装置与探针要求恢复后读回。
逐操作 PID 日志位于 `native-tests.log`、`workspace-final.log`、
`native-gpu-final.log`、`native-effect-memory.log` 和 `native-power.log`。

| 目标/行动 | 原始 - > 测试更改 - > 恢复 | 最后状态 |
|---|---|---|
| 最终完整测试的受控子进程：3952、13032、15596、16684、17748、22876、24244 | 普通优先级和已记录的允许亲和性 → 测试优先级或一个允许核心 → 精确读回原优先级与亲和性；终止子进程 | RESTORED PASS |
| 原生 GPU 探针子进程：18064、13128、12960、14460（较早为 20804） | 普通优先级及全部 16 个允许核心 → 低优先级 → 恢复普通优先级和原亲和性，读回并终止 | RESTORED PASS |
| 受控 CPU 效果探针子进程：26912 | 普通优先级 → 较高优先级（ABOVE_NORMAL）→ 每次试验后恢复普通优先级；共 12 次，清理时读回 | RESTORED PASS |
| 首次期限测量子进程：6112 | 测试进程退出后异步引用曾使子进程存留；人工恢复普通优先级、读回并终止 | RESTORED PASS，人工恢复后通过；测试装置已改为关闭前明确完成操作。 |
| 电源控制器 | 原平衡方案 `381b4222-f694-41f0-9685-ff5bb260df2e` → 一次性克隆 `0c79499a-a1b9-4017-80ab-309e62de2072` → 精确恢复原 GUID | RESTORED PASS；只删除自建克隆，最终方案列表与原始一致。 |
| 测试线程的匿名模拟身份 | 无线程令牌 → S-1-5-7 → 调用 RevertToSelf 并读回无令牌 | RESTORED PASS |
| CPU/GPU/NPU 模型子进程会话 | 无自有会话 → 隔离推理与注册尝试 → 释放队列和会话并退出子进程；仅在尚无 VitisAI 会话时注销 | 最终探针已释放，PASS；首次失败会话与 QDQ 正例卸载崩溃保留为失败证据，随后已修复。 |
| GPU/NPU 时钟、全局功率上限、驱动配置、服务与账户 | 未更改 | 无需恢复（NOT APPLICABLE） |

**剩余恢复失败：无。** 初次延迟清理子进程及两次 NPU 子进程卸载崩溃均如上记录，
并未被最终 PASS 掩盖。电源最终读回仍为原平衡方案 GUID，方案列表也与原始状态一致。
系统进程、当前 Codex、开发工具和命令行进程均未被用作控制目标。

### 环境下载

该历史轮次新增下载十个项目内 Python 包（DirectML 及九项依赖），
放在 `env/onnx-dml-packages`，缓存放在 `env/pip-cache`。
没有运行系统安装程序或安装包。`pip --target` 只把 Python 软件包展开到项目 `env/`，
未改动系统或用户全局 Python 目录：

- `onnxruntime-directml` 1.23.0、`numpy` 2.5.3、`coloredlogs` 15.0.1；
  `flatbuffers` 25.12.19、`humanfriendly` 10.0、`mpmath` 1.3.0；
  `packaging` 26.3、`protobuf` 7.36.2、`pyreadline3` 3.5.6、`sympy` 1.14.0。

既有 Python、Rust、Zig、CPU ONNX 软件包、Ollama 模型、AMD 驱动及 WinML
执行提供者均被复用。没有下载 Ollama 模型，也没有安装 NPU 开发工具包。
`downloads.json` 记录了准确的项目内包清单。最初查询 DML 1.23.2 失败；
实际放在项目内的版本为 1.23.0。

### 文件完整性和剩余验收

七个指定残留文件的 SHA-256 哈希不变，记录见
`residual-before.json` 与 `residual-after.json`。最初跟踪的 **605 个文件**仍在。
该清单基于受版本控制的源文件，不声称校验了每个原有的 `env/` 或 `target/` 生成文件。
没有删除、移动或重命名源码、测试、备份或历史报告。
本节以下的旧报告保留原有历史记录。

| 剩余项目 | 状况/具体原因 |
|---|---|
| 默认 FP32 策略模型的 NPU 推理 | 兼容性 FAIL，生产 NPU 推理 NOT VERIFIED。实验性 QDQ 候选模型的真实 NPU 推理为 PASS，但 1,000 个合成样本中有 16 个最高分标签不同；提升为默认模型前仍需代表性精度验证与验收。 |
| 全局 GPU/NPU 时钟、功率、温控与类型化插件原生绑定 | NOT VERIFIED；该厂商仍缺少集成的、可独立设置并读回的安全契约；类型化绑定保持默认拒绝。 |
| 另一登录用户的管道访问拒绝 | NOT VERIFIED；`kenny` 账户已启用，但当前身份没有其凭据，也未在该账户下交互运行。原生匿名拒绝与所有者自检通过；已准备跨登录客户端，待该账户执行。 |
| 原生守护进程自动设备放置与完整延迟 | NOT VERIFIED；已发现设备的 GPU/NPU 利用率不可用，修正后的特征链路因此拒绝推理。直接设备队列通过；自动路由及含持久化与 IPC 的完整期限仍是独立验收项。 |
| 硬期限、代表性效果与长期个人预测 | NOT VERIFIED；有限的 1,000 次原生调用、12 对短时受控 CPU 样本及一个进程的 60 个只读前台样本不足。仍需代表性应用、能耗及长期数据集。 |
| 通用自主工具代理 | 实现决定为 SKIPPED；无限制执行会绕过现有类型化校验，须先设计工具模式、权限与回滚语义。 |
| 内核集成 | NOT APPLICABLE；分析没有发现已测量的必要性 |
| 安装包安装 | SKIPPED，明确禁止。 |
| 服务部署、跨厂商验证与 Linux 原生运行 | NOT VERIFIED；未执行，也不暗示完成硬件或平台矩阵认证。 |

已执行操作的待批准事项：**无**。第二位实际登录用户的凭据、加速器利用率观测、
经同意的代表性应用任务、生产模型等效性，以及厂商控制契约仍是具体阻塞项。
目标产品与原生认证**尚未完成**。

### 修改或新增的源码与文档

| 路径 | 目的 |
|---|---|
| `Cargo.lock` | 登记已缓存的 Windows 依赖；未升级 MSRV。 |
| `README.md` | 说明可配置的 ONNX 后端选择。 |
| `adapters/windows/Cargo.toml` | 启用现有 Windows 软件包的 DXGI 与 SetupAPI 特性。 |
| `adapters/windows/src/hardware/capability.rs` | 原生身份与能力发现，以及精确电源接口的正确性。 |
| `adapters/windows/src/hardware/telemetry.rs` | 原生身份与能力发现，以及精确电源接口的正确性。 |
| `adapters/windows/src/power/controller.rs` | 原生身份与能力发现，以及精确电源接口的正确性。 |
| `adapters/windows/src/telemetry/provider.rs` | 原生身份与能力发现，以及精确电源接口的正确性。 |
| `ai-runtime/src/onnx/python.rs` | 显式提供者与设备选择，以及可选的 DLL 协议。 |
| `ai/src/intent_config.rs` | 通过环境变量注入 ONNX 提供者配置。 |
| `apps/aicored/src/runtime.rs` | 发现并注入已配置的原生会话队列；拒绝含糊的 NPU 绑定。 |
| `contracts/src/traits/power_controller.rs` | 精确的平台电源状态快照契约与默认恢复。 |
| `contracts/src/traits/runtime_binder.rs` | 准备阶段安全绑定器声明默认关闭。 |
| `docs/ai/INTENT_BACKENDS.md` | 同步当前源码与验收证据。 |
| `docs/architecture/KERNEL_INTEGRATION_ANALYSIS.md` | 记录内核集成必要性分析。 |
| `docs/quality/DESTINATION_REQUIREMENTS.md` | 同步当前源码与验收证据。 |
| `docs/quality/DESTINATION_VERIFICATION_REPORT.md` | 同步当前源码与验收证据。 |
| `ipc/src/transport/windows.rs` | 验证匿名主体 ACL 拒绝和令牌恢复读回。 |
| `models/policy_classifier_v1.onnx` | 导出与现有四特征、五标签模型等价的模型。 |
| `scripts/models/quantize_policy_npu.py` | 在 `env/` 中复现实验性 QDQ 候选模型并测量合成样本漂移。 |
| `tests/nativeNpu.queue.probe.example.rs` | 验证原生执行提供者支持的 NPU 队列、提供者放置和会话释放。 |
| `tests/nativeObservation.probe.example.rs` | 经同意的 Windows 适配器观测；不采集窗口标题，不写入未注册应用的记忆。 |
| `scripts/verify/cross_user_acl_client.ps1` | 第二登录用户只读管道拒绝客户端；要求不同 SID。 |
| `scripts/verify/cross_user_acl_server.ps1` | 有界、仅所有者可访问的测试守护进程及所有者正例检查。 |
| `runtime-router/src/binder.rs` | 仅会话型绑定器声明准备阶段安全。 |
| `runtime/src/adaptive_loop.rs` | 在慢侧准备安全会话绑定；为加速器路径提供推理输入。 |
| `runtime/src/lib.rs` | 导出排队会话绑定器。 |
| `runtime/src/model_binder.rs` | 声明仅模型绑定器在准备阶段安全。 |
| `runtime/src/queued_model_binder.rs` | 使用现有调度器新增有界真实模型队列执行。 |
| `scripts/audit_v4_static.sh` | 排除依赖制品，同时检查所有已声明测试与样本引用。 |
| `scripts/models/export_policy_onnx.py` | 导出现有策略权重与元数据。 |
| `scripts/models/onnx_intent_runner.py` | 提供明确的节点放置证据、本地执行提供者生命周期与缓存，以及失败会话的安全卸载。 |
| `scripts/models/test_onnx_bridge.py` | 回归真实 CPU/GPU/NPU 执行和重复的默认模型 NPU 失败清理。 |
| `scripts/verify/native_power_snapshot.ps1` | 封装受控克隆电源方案的设置、读回和精确恢复。 |
| `tests/Cargo.toml` | 登记保留与新增的原生探针；未移除测试。 |
| `tests/hardware.telemetry.unit.test.rs` | 验证受控目标与恢复、实测数据，或修正硬件与错误断言。 |
| `tests/native.fixture.rs` | 验证受控目标与恢复、实测数据，或修正硬件与错误断言。 |
| `tests/native.power.fixture.rs` | 验证受控目标与恢复、实测数据，或修正硬件与错误断言。 |
| `tests/nativeGpu.queue.probe.example.rs` | 验证受控目标与恢复、实测数据，或修正硬件与错误断言。 |
| `tests/nativePower.snapshot.probe.example.rs` | 验证受控目标与恢复、实测数据，或修正硬件与错误断言。 |
| `tests/ownedProcess.probe.example.rs` | 验证受控目标与恢复、实测数据，或修正硬件与错误断言。 |
| `tests/pipeline.adaptiveComputer.integration.test.rs` | 验证受控目标与恢复、实测数据，或修正硬件与错误断言。 |
| `tests/pipeline.executePolicy.integration.test.rs` | 验证受控目标与恢复、实测数据，或修正硬件与错误断言。 |
| `tests/process.controller.contract.test.rs` | 验证受控目标与恢复、实测数据，或修正硬件与错误断言。 |

删除、移动或重命名的源文件：**无**。

---

## 续作：2026-09-27（桌面后端、授权与调度）

本轮只修改 `C:\Users\12282\Desktop\AICore_V4_Windows`，未同步或修改 WSL 副本。
这是目标设计的代码开发；这些新增内容不应被称为 V4 缺陷修复，
也不表示目标产品与硬件验收已经完成。

### 交付的代码和证据

| 要求 | 已实施的行为 | 验证范围 |
|---|---|---|
| ONNX／本地 JSON 意图 | 可配置的运行时词汇表与类型标签适配器；已提交的加权 JSON 与对应的真实 MatMul/Softmax ONNX 模型；隔离的仅 CPU Python ONNX 运行时工厂，具备模式校验、有界输出和子进程超时。 | 真实 JSON 与 ONNX CPU 推理；模拟守护进程意图 IPC。 |
| 高级 GPU/NPU 调度 | 有界准入，按优先级、最早期限和 FIFO 排队；已注册的能力矩阵与后端槽位；温控保护、取消与过期；经安全执行器发放不可变单次派发租约。 | 模拟准入、温控、容量和派发测试；不声称原生设备调度已验证。 |
| 代理协作 | 并发且有界的提供者提案、隐私过滤、置信度投票、分歧拒绝与最短过期时间；守护进程组合选用本地 JSON 与 Ollama。 | 三个慢速模拟代理共享同一实际期限；真实组合 IPC 断言两个提供者均参与。 |
| 效果反馈 | 共享的 FeedbackPolicyEngine 包裹守护进程 AI 策略生成与规则回退；可信的最近已应用策略入口拒绝模型执行证据；确认退化后最长一小时内绕开匹配的主策略。 | 确定性退化与模型拒绝测试，以及独立的受控子进程观测；没有通用守护进程工作负载采集器，也不声称产品收益。 |
| 跨主版本 IPC | 明确协商 1.0/2.0；第二版要求握手会话，使用对应的类型化载荷及选定版本的确认与响应；拒绝未知主版本和未握手的第二版请求。 | 原生同用户 Windows 命名管道第二版往返；契约测试与 Linux 目标代码检查。 |
| 工件与 SID 策略 | 传输层以只读方式解析可执行文件 SHA-256，以及 Windows 令牌 SID 或 Linux UID；可选主体清单纳入逐动作身份固定检查。 | 原生同用户工件与 SID 接受、SID 不匹配拒绝；没有跨用户 ACL 或签名证明。 |
| 个人任务前准备动作 | 经同意保留的应用与小时模式生成延迟准备任务；应用时重新检查所有者、对等身份、身份固定、同意和有效期；仅进程目标跳过全局电源与运行时绑定。 | 守护进程加模型的同意撤销及应用测试；延迟队列、取消、过期和保护清理测试；空的仅进程目标为 NoOp。 |
| 文档 | 当前需求清单、IPC 规范、意图配置、调度与反馈、安全测试边界。 | 与源码及本报告保持一致。 |

JSON/ONNX 资产是人工编写的分类器，不是训练过的生成式 LLM 或已校准精度的模型。
ONNX 子进程会话不持久。规则回退仍可能生成类似的资源配置，
不能保证逆转已观测到的退化。流水线延迟与模型效果不等于应用性能改善证据。

### 项目一级的核查

所有命令使用既有 Rust **1.75.0** 工具链、项目内 `env/cargo-home`，
并指定 `--locked` 和 `--offline`。未升级最低支持版本。

| 命令/检查 | 实际结果 |
|---|---|
| `cargo check --workspace --all-targets --locked --offline` | **PASS**，退出码 0；`env/validation/desktop-check-final.log` |
| `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings` | **PASS**，退出码 0；`env/validation/desktop-clippy-final.log` |
| `cargo fmt --all -- --check` | **PASS**，退出码 0 |
| 下方的安全隔离工作空间测试命令 | **PASS**，退出码 0；执行 143 个测试目标，**719 通过、0 失败、0 忽略、5 过滤**；`env/validation/desktop-safe-tests-final.log` |
| `aicored --once`，使用 `AICORE_EXECUTION_MODE=model` 与隔离路径 | **PASS**，退出码 0；一次模型调用、一次模型成功、零次失败；`env/model-final-1790515502945` |
| `scripts/verify/simulated_platform_windows_smoke.ps1` | **PASS**，退出码 0；10 次调用、5 次模拟成功、0 次失败，并查询管道状态与调用记录；`env/windows-smoke-1790515504398` |
| `simulated_platform_ollama_intent.ps1 -Backend json/onnx/ollama/ensemble`，四种后端分别运行 | 四项均 **PASS**；组合置信度依次为 0.9522384、0.9522384、1、0.97611916；组合测试要求两个提供者均参与。本机 Ollama 模型为 `llama3.2:3b`。 |
| `simulated_platform_ollama_intent.ps1 -Backend json -RejectSid` | **PASS**；同用户原生管道拒绝不匹配的 SID。 |
| 解析九个仓库 PowerShell 脚本和两个新模型 Python 脚本的语法树 | **PASS**，退出码 0；未执行安装或控制脚本。 |
| 适用的结构审计 | **PASS**；Cargo 文件、文档和许可文件存在，没有目标目录、模型占位符、嵌套测试或过时版本文档链接。 |
| `cargo check --workspace --all-targets --target x86_64-unknown-linux-gnu --locked --offline` | 最终运行 **PASS**，退出码 0，使用项目内便携式 Zig 交叉编译器；`env/validation/linux-cross-check-final.log`。最初因缺少编译器 **FAIL**，退出码 101，日志保存在 `env/validation/linux-cross-check.log`。 |
| `cargo clippy --workspace --all-targets --all-features --target x86_64-unknown-linux-gnu --locked --offline -- -D warnings` | **PASS**，退出码 0，使用相同项目内编译器；`env/validation/linux-cross-clippy-final.log`。只完成编译与静态检查，未执行 Linux 原生测试。 |
| Linux 目标的 contracts/core/api/ai/ai-runtime/ipc/task-runtime 全目标检查 | **PASS**，退出码 0；`env/validation/linux-cross-code-check.log`。这不是整个 Linux 工作空间检查，也不是 Linux 运行结果。 |

精确孤立的套件命令 :

```powershell
$env:AICORE_SMOKE_SECONDS='1'
cargo test --workspace --all-targets --locked --offline -- --skip controller_handles_bad_pid_gracefully --skip executor_runs_and_reports_outcomes --skip full_loop_runs_and_records_metrics --skip history_disabled_by_default_means_no_records --skip history_enabled_records_tick
```

这五个测试仍在源码中启用；该次运行**使用了过滤**，不能写成无过滤测试通过。
这些测试当时可能使用非受控进程目标或系统电源控制器，无法保证精确恢复。
套件中的安装包与服务测试为静态检查或模拟测试，不是安装包或 SCM 安装。
Linux 专用用例没有在这台 Windows 主机上获得原生运行证据，也没有借助 WSL 运行或同步。

初次 ONNX 守护进程冒烟测试 **FAIL**：提供者加载期间，状态发布早于 IPC 绑定。
冒烟脚本现仅重试启动阶段的连接失败，绝不重试授权或推理失败。
最终 ONNX 运行 **PASS**；先前的测试和构建错误由上述最终命令取代，而非伪装为成功。

### 本地业务和恢复

用户授权明确允许本地控制这种延续.
`cargo run -p aicore-tests --example owned_process_probe --locked --offline`
是一个单独引用的项目建造的探测器,而不是安装器或自动单元
它只针对刚出生的孩子,拒绝继承
非普通班级。

| 目标 | 修改 | 应用/ 回读 | 恢复 |
|---|---|---|---|
| 子进程 PID 15228, `owned_process_probe.exe --child` | 常规 + AboveNormal (PolicyPriority* 高级) | **PASS** | 正常恢复并读回;清理结果为 `Ok(Normal)`,儿童终止/康复 |
| 子进程 PID 19436, 相同的自有探测器 | 常规 → AboveNormal | **PASS** | 正常恢复并读回; 清理结果为 `Ok(Normal)`, 终止/ 取消; `env/validation/owned-process-final.log` |

** 恢复失败:无。 ** 无亲和、系统动力装置, GPU/NPU,
注册、服务或持续主机配置被修改。没有安装器
正在运行。测试守护进程是抓取路径模拟程序,并被终止
根据他们自己的剧本。

最终的原生探测器在进行对齐观测前将工作量加热,使用
儿童完成期及目标父母+儿童记忆读数
`ITelemetryProvider::collect_process`,然后输入真实的比较
政策包装器。 终点内存差值具有 MiB 分辨率, 而不是一个峰值记忆
最终延迟为3.8752毫秒和3.8956毫秒;评估为:
** 缺乏结论**,不是经过核实的绩效收益。
没有,没有慢AI的工人在探测器中被即时处理,所以不能
认证本地双轨制最后期限。

### 环境下载

下载的软件包和缓存被限制在 `env/onnx-packages` 和
`env/pip-cache`; 使用的 pip 临时文件 `env/`重新使用现有的
`C:\Users\12282\anaconda3\python.exe`; 无解释器, Rust工具链,
Ollama型号、安装器或驱动程序被下载。 Python 软件包目录 :

- `onnxruntime` 1.23.2、`onnx` 1.20.1、`numpy` 2.3.5
- `coloredlogs` 15.0.1、`flatbuffers` 25.12.19、`humanfriendly` 10.0
- `ml_dtypes` 0.6.0、`mpmath` 1.3.0、`packaging` 26.3
- `protobuf` 7.36.2、`pyreadline3` 3.5.6、`sympy` 1.14.0、`typing_extensions` 4.16.0。

这些是项目本地的 `pip --target` 依赖关系。 孤立的跑者确实如此 。
生成 ONNX 模型和验证日志
属于项目制品,而非下载环境。

为了解决Linux C跨构造阻塞器,还下载了便携式
** Zig 0.12.1 Windows x86_64 ZIP**(76,470,574字节) 出自:
[官方下载索引](https://ziglang.org/download/index.json)至
`env/zig-0.12.1-windows.zip`,然后提取为 `env/zig-toolchain`.
SHA-256根据官方指数进行了核实:
`52459b147c2de4d7c28f6b1a4b3d571c114e96836bf8e31c953a7d2f5e94251c`.
这是编译器存档, 不是安装器。 它的全球/ 本地缓存覆盖
两者都是当地项目(`env/zig-global-cache`, `env/zig-local-cache`。没有系统
PATH 生成 `env/zig-linux-cc.cmd` 地图 cc-rs 的 Rust 目标
拼写到 Zig 的 C 目标拼写; 初始原始/ 拼写尝试失败 。
最后的交叉检查用途 :

```powershell
$env:CC_x86_64_unknown_linux_gnu="$PWD\env\zig-linux-cc.cmd"
$env:AR_x86_64_unknown_linux_gnu="$PWD\env\zig-toolchain\zig-windows-x86_64-0.12.1\zig.exe ar"
$env:ZIG_GLOBAL_CACHE_DIR="$PWD\env\zig-global-cache"
$env:ZIG_LOCAL_CACHE_DIR="$PWD\env\zig-local-cache"
cargo check --workspace --all-targets --target x86_64-unknown-linux-gnu --locked --offline
```

此检查当前 Linux 编译的桌面来源。 它不执行
Linux程序,修改WSL,或认证原生的Linux硬件.

### 文件完整性和更改的文件

所有591个基线跟踪文件都保留下来。
SHA-256 数值不变;在清单和散列之前/之后
`env/validation/tracked-files-{before,after}.txt` 和 (简体中文)
`env/validation/residual-hashes-{before,after}.json`. 现有未跟踪
`docs/assets/` 架构材料保存完毕. ** 删除原始文件 :
无。 ** 没有删除、忽略或削弱任何测试。

更改/添加源、测试、模型、配置和文件路径(不)
包括环境/缓存/构建产出或原有建筑资产:

```text
Cargo.lock
README.md
adapters/windows/src/telemetry/provider.rs
ai-runtime/src/onnx/mod.rs
ai-runtime/src/onnx/python.rs
ai/Cargo.toml
ai/examples/configured_intent.rs
ai/src/intent.rs
ai/src/intent_config.rs
ai/src/lib.rs
ai/src/orchestration.rs
ai/src/runtime_intent.rs
api/src/identity_policy.rs
apps/aicore-cli/src/main.rs
apps/aicored/src/ipc_handler.rs
apps/aicored/src/runtime.rs
apps/aicored/src/scheduler.rs
contracts/src/execution/targets.rs
contracts/src/ipc/protocol.rs
contracts/src/traits/task_queue.rs
contracts/src/traits/telemetry_provider.rs
core/src/accelerator_scheduler.rs
core/src/executor/policy_executor.rs
core/src/lib.rs
core/src/model.rs
core/src/policy/feedback.rs
core/src/policy/mod.rs
docs/ai/INTENT_BACKENDS.md
docs/contracts/IPC_PROTOCOL.md
docs/quality/DESTINATION_REQUIREMENTS.md
docs/quality/DESTINATION_TEST_SAFETY.md
docs/quality/DESTINATION_VERIFICATION_REPORT.md
docs/runtime/SCHEDULING_AND_FEEDBACK.md
ipc/src/client.rs
ipc/src/identity.rs
ipc/src/server.rs
ipc/src/transport/mod.rs
ipc/src/transport/windows.rs
models/intent_classifier_v1.json
models/intent_classifier_v1.onnx
runtime/src/adaptive_loop.rs
scripts/models/export_intent_onnx.py
scripts/models/onnx_intent_runner.py
scripts/verify/simulated_platform_ollama_intent.ps1
task-runtime/src/queue.rs
tests/Cargo.toml
tests/modelNORealPC.execution.unit.test.rs
tests/ownedProcess.probe.example.rs
```

### 剩余接受和阻断依赖

- NOT VERIFIED:** 本地的GPU/NPU 调度,插件硬件回读和
  热/ 功率优化; 没有经过认证的设备执行/ 背面后端
  可用。框架已执行,但守护进程硬件排程绑定
  而真实的交叉推论仍然依赖于实际的后端.
- NOT VERIFIED:** Windows 交叉用户 ACL 拒绝,签名/装入图像
  证明书完整的失效对端/重联矩阵本地硬期限
  真实的个人记忆精度/效果和服务生命周期。
- **NOT VERIFIED** 代表效应/控制组群,具体工作量
  值得信赖的集热器集成和能源/高峰期预算。
  孤立的测量不能接近产品的有效性。
- 最初的Linux C编译器阻塞器是**已解决 ** 通过工程本地便携式
  Zig; 最终全数 Linux 工作空间编译和 Clippy 检查通过 。 Linux
  本地执行/实际硬件残存**NOT VERIFIED**.
- **SKIPPED:** 上述未过滤的本地控制测试;包安装
  仍然被禁止,没有尝试。
- 通用自主工具代理协作和基因组 ONNX LLM 适配器
  不通过分类组合执行。
  界定后端/合同和单独的建筑范围决定。
- ** 授权申请:无**实际执行的行动;
  明确的用户授权覆盖自有儿童探测器。
  安装操作仍然被禁止。

本批通过 Windows 代码/同位素/模型/模拟门。它确实通过
** 不** 宣布全部目的地产品或本地硬件认证
现有要求库存被刻意保留为PARTIAL
实际整合、代表性数据或平台接受度仍然缺失。

## 续:2026-09-24(桌面密码层关闭)

此续作仅更改了`C:\Users\12282\Desktop\AICore_V4_Windows`.
WSL 工程没有同步或修改。 桌面构建仍然保留
使用既有 Rust 1.75 工具链和项目本地 `env/cargo-home` 离线完成。
没有下载或安装环境，也没有调用服务或宿主机控制设置器。

应用注册现在可以选择使用操作员持有的制品固定策略文件
（`AICORE_APP_POLICY_PATH`）。守护进程通过只读操作系统进程接口，
从已认证对端的可执行文件解析 SHA-256 摘要，并按应用操作检查授权。
身份缺失、失效、不可读或未获批准时默认拒绝。这只能证明磁盘上的
可执行文件摘要和进程身份约束，**不能**替代签名映像验证或 Windows
原生跨用户 ACL 认证。IPC 支持明确的版本协商：
握手降级; 不支持的线条主要版本仍然失败 。

已有的插件激活路径已输入 GPU/NPU电源控制消息,
操作员针头、现场许可、能力检查和独立回放检查。
模拟测试涵盖成功和失败的关闭。 没有真正的控制插件或
经认证的后端已激活。 本地 Ollama 意图提供者使用
`AICORE_OLLAMA_MODEL` (默认) `llama3.2:3b`和一个仅循环的终点;
IPC 意图 API 返回已验证的语义建议,而不是直接激活。
IPC 处理器工作运行在 Tokio 的屏蔽池如此可执行的散列和本地
模型延迟性不占据异步反应堆。模型定时观测仍然保留。
标记为 `Model` 且不能建立现实世界效果。

安全 Windows 工作空间测试命令不包括5个可以保留的测试
到达主机进程或电源设置器。它通过失效之后 Windows GPU 测试
已更正为实际能力合同。 Ollama
提供者实例使用
已运行 `llama3.2:3b`. 初始模拟的-守护进程到 Ollama 尝试
在无法使用外部服务时拒绝连接失败 。
在它再次可用后, 同样的模拟守护进程, Windows 命名管道
IPC,操作员仅可读的CLI可执行的SHA-256,以及`llama3.2:3b`
意图路径为 **PASS**。结果证明：
测试时的本地配置,而不是其他模型或实际激活.

| 最终桌面检查 | 成果和范围 |
|---|---|
| `cargo check --workspace --all-targets --locked --offline` | **PASS**,退出码 0 |
| `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings` | **PASS**,退出码 0 |
| `cargo fmt --all -- --check` | **PASS**,退出码 0 |
| `cargo test --workspace --all-targets --locked --offline -- --skip controller_handles_bad_pid_gracefully --skip executor_runs_and_reports_outcomes --skip full_loop_runs_and_records_metrics --skip history_disabled_by_default_means_no_records --skip history_enabled_records_tick` | **PASS**,退出码 0:141个目标,711个通过,0个失败,0个被忽略,5个被过滤;完整日志 `env/validation/safe-tests-final.log` |
| `scripts/verify/simulated_platform_windows_smoke.ps1` | **PASS**,退出码 0:10个控制周期,5个模拟成功,0个失败,命名管状态/控制周期 |
| `scripts/verify/simulated_platform_ollama_intent.ps1 -Model llama3.2:3b` | **PASS** 在最后绑定的运行上,退出码 0:只读可执行的插针和有把握的编译意图 1,在命名的管道上;初始未绑定的尝试**FAIL** 连接被拒绝时 Ollama 无法使用 |

当地实际工作量影响 Windows ACL 跨用户拒绝, 签名图像
验证, 本地控制期限, 认证的插件控制回读,
GPU/NPU/动力激活,Windows服务/SCM和其他型号后端仍然保留.
**NOT VERIFIED** 它们不能从模拟或模拟成功中推断出来。

## 续:2026-09-24(Windows桌面平台模拟器)

该项目从WSL复制到
`C:\Users\12282\Desktop\AICore_V4_Windows` 不复制 Linux 构建输出。
没有运行安装器或服务。 已存在 Windows 破损1.75和 MSVC 二进制
第一个离线构建找到了 Windows 用户的货运
缓存不完整( E)`base64ct` 缺少); 已存在的本地 WSL 货物缓存/索引
被复制到桌面工程的 `env/cargo-home`。所有后续的货运
命令使用了该目录和 `--offline`. ** 没有网络环境下载
已提出或核准。 **

新的计划 Windows 被俘虏了 Windows 适配器仅读方法
并记录在 `docs/platform/PLATFORM_SIMULATOR.md`。其内容 SHA-256号
`de6a6903f51c49619059edf4e55e90dd7772a98ddde6c92a953ad3556ffa1cec`.
它包含10个过程特性,具有更明显的优先地位和亲和性;
识别字符串已编辑 。 此主机未生成样本 GPU/NPU 且无
已确认内置电源配置 。 这些不支持的路径没有被覆盖
与发明的值。 Windows 现在建造这个测试样本; Linux 建筑物保存
他们的 Linux/WSL 捕捉。 `system_smoke` 测试现在使用孤立的遥测和
没有控制器,声称`successful + failed + noop = total`,并核实所有
它的控制周期是中性的NoOp。它的三项测试已不再从中过滤。
Windows 程序优先性能力声明
而亲和性写入现在是`Conditional`关于目标访问权限,匹配
实际数PID 获得/获得合同; 失效 `Unsupported` 测试是固定的。

| 检查 | 成果和范围 |
|---|---|
| Linux `cargo check --workspace --all-targets --locked` | **PASS**,退出码 0 |
| Linux `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | **PASS**,退出码 0 |
| Linux `cargo fmt --all -- --check` | **PASS**,退出码 0 |
| Linux `bash scripts/verify/model_safe_workspace_tests.sh` | **PASS**,退出码 0:140项测试目标,701项通过,0项失败,0项忽略,40项过滤的原生控制/主机敏感测试 |
| Windows 桌面 `cargo check --workspace --all-targets --locked --offline` | **PASS**, 退出码 0, 本地 MSVC 构建 |
| Windows 桌面 `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings` | **PASS** 退出码 0; Windows 仅更正 Clippy 缺陷 |
| Windows 桌面 `cargo fmt --all -- --check` | **PASS**,退出码 0 |
| Windows 桌面 `cargo test -p aicore-adapter-simulated-platform --all-targets --locked --offline` | **PASS**, 退出码 0: 4 模拟单元测试; 没有主机设置器 |
| Windows 桌面 `cargo test -p aicore-daemon simulated_platform_tests --locked --offline -- --test-threads=1` | **PASS**，退出码 0：4 项测试，覆盖慢提供者截止期限、过期、回滚和 SafeMode；使用 Windows 采集样本。 |
| Windows 桌面 `cargo test -p aicore-ipc --all-targets --locked --offline` | **PASS**，退出码 0：2 项测试，包括真实同用户命名管道握手、Ack 和当前对端身份。 |
| Windows 桌面 `cargo test -p aicore-tests --test system_smoke --locked --offline` | **PASS**,退出码 0:3个隔离 NoOp 冒烟测试 |
| Windows 桌面只读 `capability_detect_unit` 和 `telemetry_real_data_unit` 目标 | **PASS**，退出码均为 0：分别运行 8 项和 5 项测试；未调用设置器。 |
| Windows 桌面 `scripts/verify/simulated_platform_windows_smoke.ps1` | **PASS**，退出码 0：连续运行 10 个控制周期，5 次模拟执行成功、0 次失败；CLI 的 `status` 和 `ticks` 使用隔离文件与真实命名管道。 |
| 未过滤的本地控制工作空间测试和 Windows 交叉用户 ACL 拒绝 | SKIPPED / NOT VERIFIED** 现授权及可用身份设置 |

Windows 同用户命名管道和本地对端身份路径现在有原生
运行时间证据,而Windows服务/SCM,交叉用户ACL否认,真实
重点/力量/力量写道: GPU/NPU 控制、实际影响和原生影响
控制期限保持不变**NOT VERIFIED**. 模拟成功不是本地的
硬件认证。 没有请求或更改系统配置 。

## 续:2026-09-24(抓好平台模拟器)

目标完成情况**NO / NOT VERIFIED** 续篇增加
`adapters/simulated-platform` 并选用
`AICORE_EXECUTION_MODE=simulated-platform`. 适配器执行现有的
它的Linux/WSL
样本是由实际只读的适配器调用和编辑器生成的
字符串; 见 `docs/platform/PLATFORM_SIMULATOR.md` 来源和来源 SHA-256.
守护进程连续平台模式使用模拟 `PlatformTickDriver`
遥测、政策执行、条件更新和运行时间绑定。
子进程激活在此模式下被禁用。 单点守护进程路径是
仍然同步。

已采集 PID 的读回通过四项守护进程集成测试，耗时约 250 毫秒
提供方带有快速中性控制周期和过期计划拒绝, 新鲜
已准备好的政策应用程序,呆滞的回读导致无假的回滚
成功, 重复写入失败, 输入 SafeMode. 三个模拟器单元
为恢复亲缘关系、缺乏硬件/电源和固定设备而通过测试
`PluginActivationManager::replace_active` 现在要求
登记册在停止旧进程前验证替换项；登记册
提交和旧进程停止共享同一把锁。现有模拟
校正后通过升级/回滚测试。

| 检查 | 成果和范围 |
|---|---|
| `cargo check --workspace --all-targets --locked` | **PASS**,退出码 0, Linux 工作空间 |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | **PASS**,退出码 0 |
| `cargo fmt --all -- --check` | **PASS**,退出码 0 |
| `bash scripts/verify/model_safe_workspace_tests.sh` | **PASS**,退出码 0:139个目标,697个通过,0个失败,0个被忽略,43个被过滤的本地控制/主机测试 |
| `cargo test -p aicore-adapter-simulated-platform --all-targets --locked` | **PASS**, 0 出口: 3 个单元测试, 捕获实例编译 |
| `cargo test -p aicore-daemon simulated_platform_tests -- --test-threads=1` | **PASS**,退出码 0:4个隔离集成测试 |
| `bash scripts/verify/simulated_platform_smoke.sh` | **PASS**,退出码 0: 1 `--once` 控制周期和 30 个连续控制周期在临时目录中; 最终运行有 17 个成功的模拟执行, 0 失败, 剩余控制周期中性/NoOp (早先的一次运行有16次成功;时间影响计数) |
| 隔离的 `AICORE_EXECUTION_MODE=model target/debug/aicored --once ...` | **PASS**,退出码 0:本地 JSON 模型已装入, 一个模型控制周期, 没有主机控制 |
| 只读 Linux 遥测、能力、硬件遥测和硬件能力目标 | **PASS**,退出码 0:20次测试,跨越4个目标;固定捕获是一个单独的只读适配器运行 |
| `cargo check -p aicore-adapter-simulated-platform -p aicore-plugin --all-targets --locked --target x86_64-pc-windows-msvc` | **PASS**, 退出码 0: 只编辑源代码 |
| `cargo check -p aicore-daemon --all-targets --locked --target x86_64-pc-windows-msvc` | **FAIL**,退出码 1 WSL: 捆绑 `libsqlite3-sys` 要求 MSVC `lib.exe`,此处不在此;否 Windows 推断守护进程行为 |
| `bash scripts/audit_v4_static.sh`, `bash scripts/audit_v32_static.sh`, `bash -n scripts/verify/simulated_platform_smoke.sh`, `git diff --check` | **PASS**,退出码 0;V4审计解析110份申报的Rust测试文件 |
| 未过滤的 `cargo test --workspace --all-targets --locked` | **SKIPPED**:43项经过审查的测试可使用本地控制或主机敏感路径;测试仍然启用 |

七个指定的受保护残留文件保留修改前的 SHA-256，且
`git ls-files --deleted` 为空。初始完整文件清单记录在 `/tmp`，
但该临时文件在最终比较时已不存在，因此整棵工作树的前后差异为
**NOT VERIFIED**。没有删除受跟踪文件；新增了软件包、样本、冒烟脚本
和模拟文档。没有请求或批准宿主机配置修改，也没有运行安装程序或服务。
Windows 本地 Rust 版本探测器曾误用 `rustup` 的部分安装工具链，
修理/下载路径,所以 Windows 停止了内建执行; WSL
以上 MSVC 目标编译结果只是当时 Windows 代码层面的证据，

其余的目的地差距是相同的实质性产品接受率
下表中的差距:无实际当地 LLM 型号/提供者或 GPU/NPU 模型
后端,无操作员核准的应用程序制品/ SID 政策,无对齐
观察到的工作量效益或政策反馈,没有先进的本地 GPU/NPU电源
动因证据,无原生 Windows IPC/服务或实际控制期限
证书。 WSL 固定无可控 GPU/NPU 或电源简介;
模拟成功不能取代这些观察。
请求必须列出一个稳定的 PID/名称、目标设备、确切的更改、状态前,
范围、退缩和等待明确批准。

## 目前续作:2026-09-24(未关闭输入和只读验证)

目标完成时间为 **NO / NOT VERIFIED** 说明 Linux 硬件执行器
现在只接受 `drm:card<digits>` 设备 IDs 和已知的工作量提示。
未知的提示和电源盖返回 NotSupported 在任何时间之前 GPU 写作。 Linux
和 Windows 电源适配器不再解释陌生 OS 电力计划
作为平衡: 回读报告 CapabilityUnavailable. 键入的插件优先级
桥拒绝 PID 在发出请求前 0,因为 PID 0有特别的
某些平台上的流程控制意义。这些是合同/安全规定;
Linux IPC现在也捕捉到对等方
可执行设备/模式,并在每个应用程序范围的请求中重新检查它。
替换的可执行文件会失去授权, 而不可读的身份是
拒绝该请求而不将所有权转让给另一个同行。
这是流程艺术连续检查,不是操作员批准的
应用程序签名/ SID 策略。 现在一个孤立的可观测工作负荷助手
单调时钟和叶片的呼叫者自主工作取得成功
它的CPU固定是本地实际计算,
但没有进行政策干预或组合控制;福利依然存在
未经核实的个人记忆同意/保留 SQLite 仅使用的测试
内部或孤立的测试样本,而不是个人用户数据。

| 当前检查 | 成果和范围 |
|---|---|
| `cargo test -p aicore-adapter-linux --lib --locked` | PASS, 退出码 0: 5次测试,包括3次新的纯故障关闭测试; 无控制调用 |
| `cargo test -p aicore-plugin activation::tests::typed_priority_control_requires_exact_operator_pin_and_fails_closed --locked` | PASS, 退出码 0: 1 模拟测试; PID 0 不引起主机请求 |
| `cargo check -p aicore-adapter-windows --all-targets --locked --offline --target x86_64-pc-windows-msvc` | PASS, 退出码 0: 只编译代码 |
| `cargo test -p aicore-ipc --lib --locked` | PASS, 退出码 0: 9 测试, 包括更改可执行身份和按请求拒绝 |
| `cargo test -p aicore-daemon --bin aicored --locked ipc_handler::tests::` | PASS,退出码 0: 6 隔离 IPC 任务/所有者测试 |
| `cargo test -p aicore-core effect::tests:: --locked` | PASS,退出码 0: 4 效果测试,包括测量到的孤立测试 CPU 工作量和失败的工作负荷拒绝 |
| `cargo test -p aicore-memory personal::tests --locked` 加号 SQLite/保留目标 | PASS, 退出码 0: 15 同意, 保留和储存测试使用孤立数据 |
| `cargo check --workspace --all-targets --locked` | PASS, 退出码 0 |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | PASS, 退出码 0 |
| `cargo fmt --all -- --check` | PASS, 退出码 0 |
| `bash scripts/verify/model_safe_workspace_tests.sh` | PASS, 退出码 0: 137目标, 690通过, 0失败, 0 忽略, 43 在所有源更改后过滤 |
| `AICORE_EXECUTION_MODE=model cargo run -p aicore-daemon --bin aicored --locked -- --once ...` | PASS,退出码 0: 本地 JSON 模型已装入, 7 模拟结果在最近孤立运行中; 没有硬件控制 |
| 选定只读的 Linux 能力、遥测、硬件和电查询目标 | PASS: 跨越五个目标的21次测试; 没有引用平台设置器 |
| 真实平台模式控制控制周期, 本地的 Windows 管道和真实惠益 | SKIPPED / NOT VERIFIED;见以下操作前检查部分. |

平台控制操作前检查: ** 未请求批准, 没有主机配置
更改 **. 一个普通的平台模式控制周期可以调用`setpriority`进行检测
前景 PIDs, `sched_setaffinity` 预览/背景 PIDs 如果一个
策略提供了一个核心掩码, `setpriority(..., nice=19)` 用于节奏
背景 PIDs, Linux 电源配置 sysfs 写入, 和 AMD GPU 性能
如果可用, sysfs 写 。 WSL 主机目前没有 DRM 纸牌, ACPI
平台简介或 CPU 州长路径,但进程目标来自直播
遥测快照,可以在控制周期之间更改。没有稳定、孤立的 PID/name
已选中, 因此用户的确切目标批准条件尚未确定
控制试验必须首先冻结一个孤立的目标PID及其
基线优先级/强度;它还必须捕捉任何现有设备和动力
状态,指定请求的值、范围和回滚,然后明确请求
现有政策倒退不是证据
以恢复主机的原始设置。
并且效果接受仍然开放。


## 目前续作:2026-09-24(只读原生证据).

目标完成情况**NO / NOT VERIFIED** 使用的这种延续
仅读 Linux 主机观察的新授权。它没有运行
安装器、服务或平台控制控制周期。

- 连续平台模式现在使用一个捆绑的慢准备工人。
  遥测/历史/预测/政策路径不再阻碍投票;
  绑定器在计划到期后留下 SafeMode检查,因为它
  能够写入 GPU/NPU 状态。模拟测试显示 250 ms 慢遥测调用
  不要阻止快速的民意调查并且一个过期的计划从不引用绑定器。
  绑定器平台、执行器、事件发布和状态文件仍然可以延迟
  执行方; 没有硬期限或实控通行证。
- Linux 进程优先顺序、亲和性、能标注和 AMD GPU 写入能力
  声明现在说明所需的运行时间许可/司机条件。
  单是当前 sysfs 路径不报告为无条件的写入支持 。
  CPU/AI 现在报告没有推断输入的绑定器路线 `executed=false`;
  GPU/NPU设备IDs没有合成OS特有的倒置.
- 定期守护进程收割者检查确认 IPC 同行在屏蔽上死亡/被重用
  工人,取消其活动任务,并清除L1/注册绑定。
  已准备的政策应用路径在绑定前重新检查任务撤销。
  仍为加工身份,而非操作者核准的应用制品身份。
- 插件 `process.priority_v1` 现在有一个输入的请求/答复和一个精确
  `Adapter`/`process.control`/操作员 `allow_control` 固定策略门。 平台模式
  通过 `ProcessSelection` 绑定一个唯一活动的插件; 它的失败确实存在
  不触发本地写入。 独立的平台控制器提供
  后置条件返回。 此路径只包含模拟测试; 没有外部控制
  插件已执行,并且 GPU/NPU/ 电源插件控制仍然未绑定 。
- 只读 Linux 适配器、硬件能力和遥测/文本测试
  被传送到这个 WSL 主机上。这些确认的读数是可用的,结构上是
  有效; 它们不是独立的传感器校准。 一个模型模式 `--once`
  运行装入本地的JSON模型,并应用7个模拟结果.
- 源头审查确认,现有的隔离过滤器已排除
  GPU `release()` 测试,可以写入 AMD sysfs. 仅读检查找不到
  相应 GPU 在此写入路径 WSL 主机。 没有真设备控制结果
  从过滤运行中推断。

| 检查此续作 | 成果和范围 |
|---|---|
| `cargo check --workspace --all-targets --locked` | PASS，退出码 0 Linux 工作空间 |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | PASS，退出码 0 Linux 工作空间 |
| `cargo fmt --all -- --check` | PASS, 格式化新能力断言后退出 0 |
| `bash scripts/verify/model_safe_workspace_tests.sh` | PASS,退出 0:137个目标,684个通过,0个失败,0个忽略,43个过滤器。先前存在的过滤器已被排除 GPU `release()` |
| `cargo test -p aicore-runtime dual_loop --locked` | PASS:7个目标测试,包括平台模式慢提供器和过期模拟器 |
| `cargo test -p aicore-daemon --bin aicored --locked ipc_handler::tests::` | PASS: 6次定向测试,包括失效应用任务清理 |
| 插件激活和守护进程选择模拟测试 | PASS: 8 个插件激活测试和 1 个选择失败关闭测试; 没有改变真正的进程优先级 |
| Linux 只读适配器,硬件和能力目标 | PASS: 跨八个单独引用的目标的27个选定测试; 无控制器调用 |
| `cargo check -p aicore-ipc -p aicore-contracts -p aicore-plugin --all-targets --locked --offline --target x86_64-pc-windows-msvc` | PASS,退出码 0: Windows 只有目标代码汇编;没有本地管道运行时间证据 |
| `AICORE_EXECUTION_MODE=model cargo run -p aicore-daemon --bin aicored --locked -- --once ...` | PASS，退出码 0,本地 JSON 模型已装入; 7 ** 模拟结果; 没有主机控制 |
| 未过滤的工作空间 `cargo test` 和真实的平台模式控制周期 | SKIPPED:直接过程, GPU 或电源控制可能改变主机配置; 需要单独的授权 |
| 原生 Windows 命名管道运行时间 | NOT VERIFIED: 本地 Rust 1. 75 工具链缺失; 离线检查开始工具链下载并立即中断; 没有运行测试二进制或安装器 |
| `bash scripts/audit_v4_static.sh`、`bash scripts/audit_v32_static.sh`、命令行环境的 `bash -n`、`git diff --check` | PASS，退出码 0；检查了 110 个已声明 Rust 测试文件和 18 个发现的命令行脚本。 |
| 七个受保护残留文件的 SHA-256 与前后文件清单 | PASS：所有哈希值不变；没有删除既有的非 `target/` 文件，并记录了新增源文件。 |

下面的前面一节记录了先前只代码的续作,必须
不读取为当前 Windows 或硬件认证 。
包含独立验证的插件控制后置条件, 操作员固定
应用制品,配对实际工作量效应观测,本地 LLM
和 GPU/NPU 模式后端, 本地 Windows IPC平台模式
这些是目的工程,而不是 V4 缺陷关闭索赔。

在此没有授权或故意执行系统配置更改
继续。 真正的平台模式控制周期仍然 **SKIPPED** 有待明确
允许,因为其现有的适配器可能会改变进程优先级/功能,
电源简介或 GPU 性能状态 。 Windows 命名管道运行时间为
NOT VERIFIED**;Linux/WSL只读观察不能代替它.
Windows主机离线编译尝试引发锈蚀缺失的Rust 1.75
组件下载并立即中断; WSL Windows目标代码
编译后没有运行 Windows 二进制。

## 最近续:2026-09-24(仅模式/代码)

目标完成情况**NO / NOT VERIFIED** 这一延续
推进多个代码路径,但不取代产品或硬件接受。

- 持续的模型模式守护进程调度现在使用一个捆绑的慢准备
  工人和新的有效的政策交接。
  历史读取、预测和模式运行时间绑定;快速部分适用
  模式执行、核查和 SafeMode,或记录中性 NoOp.
  被丢弃的计划失败了排队的任务,而不是让它们运行。
  250ms遥测提供者没有阻断快速呼叫; 一个孤立的守护进程
  测试应用了已准备好的政策并关闭。 `--once` 和平台模式
  保留同步路径。 实际的快速/ 缓慢持续时间是分开的 。
  以微秒计量,但这些不是**** 性能效益。
- 模型模式保留运行在一个有界限的背景工作者身上。调度器的
  保留时间戳从秒修正到排程器的毫秒
  NoOp不再增加运行时间故障计量。
- 输入只读的 `telemetry.snapshot_v1` 插件绑定需要远程测量
  显示, `telemetry.read` 许可和一个操作符。 明确重新启动
  失败后并进行了`replace_active`支持模拟测试升级,
  回滚：候选插件失败时旧进程仍保持运行。平台模式
  选择一个唯一活跃的输入式遥测插件并返回平台
  关于失败的观察。 模式从未启动它; 守护进程选择有
  只有模拟/编码证据。
- IPC登记程序可恢复经确认的死/再使用程序具有约束力,
  清除其L1声明(Linux 模拟/运行时间证据, Windows 代码编译
  （二）身份约束仍停留在进程层面；
  操作员批准的应用程序制品身份和队列任务清理
  握手可以明确谈判唯一执行的
  1.0 协议; 后版本不支持操作.
- 主动同意历史可以存储一个明确注册的应用程序 ID. 聚合
  生成至少三个样本后的相同应用/UTC小时模式; 预测
  只使用匹配的任务身份来消费它们。旧的记录去序列化
  模拟整合测试涵盖由此产生的政策
  路径和历史记录。这不是预期的日程安排好处。
- 守护进程使保守的热/电岗成为可能。
  JSON 模式登记为 CPU 输入 `DeviceRuntimeCatalog` 并使用
  使用模型绑定器 。OS 矩阵
  检测设备的申报;这些不是触发器证明。 GPU/NPU 模式后端或本地端 LLM 模型为
  效果比较框架尚未对齐
  观察到的工作量/能源,因此不能要求改善。

| 当前检查 | 成果和范围 |
|---|---|
| `cargo check --workspace --all-targets --locked` | PASS，退出码 0 Linux 工作空间 |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | PASS，退出码 0 Linux 工作空间 |
| `bash scripts/verify/model_safe_workspace_tests.sh` | PASS,退出码 0:137目标,679通过,0失败,0忽略,43被审查的直播控制/主机列表过滤 |
| `AICORE_EXECUTION_MODE=model cargo run -p aicore-daemon --bin aicored --locked -- --once ...` | PASS,退出码 0: 1个模型控制周期、7个模拟结果、本地路径和无运行时间绑定动作; JSON 模型已装入 |
| 孤立的连续守护进程模式测试 | PASS: 已制定的政策应用和守护进程关闭; 这不是本地服务测试 |
| `cargo check -p aicore-contracts -p aicore-core -p aicore-ipc -p aicore-plugin -p aicore-ai-runtime --all-targets --locked --target x86_64-pc-windows-msvc` | PASS, 退出 0; 只编译代码 |
| Windows 目标包括 `aicore-runtime` | WSL中的FAIL:捆绑的SQLite构建无法找到MSVC `lib.exe`;没有Windows运行时间结论 |
| `bash scripts/audit_v4_static.sh`,`bash scripts/audit_v32_static.sh`,命令行环境的 `bash -n` | PASS, 退出码 0 |
| 未过滤的 `cargo test --workspace --all-targets --locked` | SKIPPED:43个列出的测试可以使用直播控制/主机能力 |
| 真正的硬件、服务/软件包安装和原生 Windows行为 | NOT VERIFIED 按任务边界 |

最新的续作触及了最终工作树差异中的文件,
包括`apps/aicored`,`runtime`,`plugin`,`ipc`,`contracts`,`core`,
`memory`, `tests` 没有来源或受保护的残余物
删除。 文件的前后显示在 `/tmp/aicore-destination3-files-*.txt` 下
没有显示已删除或添加的非 `target/` 路径; 货物已替换 4 607 个路径
`target/` 路径。 SHA-256支票都通过
`cargo fmt --all -- --check`外壳语法, `git diff --check` 静态
在最后回归中,审计退出0。

### 剩余的代码和接受空白

输入的插件遥测路径仍需要本地进程证据和一个
安全控制接口设计; 自动重新启动/更新政策未执行。
应用程序身份需要操作员核准的可执行文件/签名政策,
Windows 本地清理和操作兼容性超过单1.0
计划. 效应评估需要独立观察配对工作量衡量标准
在反馈影响政策之前。没有本地的 LLM 模型/运行时间,
意图 API 或 GPU/NPU 推论提供者。OS 矩阵为
尚未绑定到安全激活器或正在目标主机上验证 。
执行/事件/状态写入和平台模式没有显示
不存在测量的个性化、热或电源效益。
仍然是开放目的地要求,而不是V4缺陷索赔。

## 先前的批次记录(2026-09-23).

日期:2026-09-23. 范围来源:根目录 `destination.md`和当前.
这是** 目标完成工作正在进行**,而不是V4
仅限缺陷关闭或要求硬件认证。逐项
要求、现状、剩余空白和接受证据
`DESTINATION_REQUIREMENTS.md`.

## 这项工作已执行

- 平台执行现在区分了没有验证和通过验证
  成功流程优先、节流、亲和和权力
  配置操作通过现有控制器接收回读
  被支持的接口; 错配失败校验并输入
  `ControlTrace::is_success` 遵循其最终状态。
- Linux/Windows网络字节三角洲转换为经过一段时间的费率; 2.
  Linux 电池, AC 和电源剖面读数使用只读供电
  数据。缺失的读数仍然无法或尽最大努力。
- AI 政策电话有固定的工人/死线与规则倒置。
  仅保护策略调用; 整个目标快速/ 慢循环不是
  已执行。
- Linux IPC 套接字权限和相同UID 同行检查、握手/握手
  交换、 限制连接、 闲置超时和重复请求- ID
  拒绝被添加。 只有当
  路径仍然识别它绑定的套接字。
- 守护进程保留了它的 L1/L2 提供者。 IPC 可以声明/清除 L1 流
  任务,并对现有共享队列提交/完成/取消 L2 任务。
  无法通过任意控件控制常规队列任务。 NoOp
  执行不标记已耗尽的任务 。
- 插件激活现在使用操作员固定的清单和制品摘要,
  签名/信任检查、有限度的管理程序和乒乓/乒乓保健门。
  模型模式从不发射外部插件。只有现有的通用
  `extension.execute`接口是绑定的;硬件/提供者绑定仍然保留.
- IPC 注册应用程序 ID 联合国 OS 同伴身份和检查身份
  每个应用程序动作。 Linux 使用对端 UID/PID 启动时间控制周期; Windows 拥有者/系统
  烟斗 ACL 客户端 PID/创作时间被执行和交叉编译,
  但不是运行时间测试。
- 个性化只有在容貌许可的情况下才能载入历史
  小时工作量模式和采用短期预测。
  保留试验涵盖内膜后端。
  比较框架只接受经核实的反馈的观测指标。
- 故意代理、同意代理、多代理协调员、设备模型
  运行时间目录、提供者/OS能力矩阵、看守的精算师合同
  并增加了可选热能/电能政策警卫。
  LLM, GPU/NPU 对运行时间或硬件后端进行索赔。
- 单独的不可改变的政策交割/快速消费范围
  在模拟测试中的效果,包括过期和取消。守护进程仍然
  使用同步 `AdaptiveLoop::tick`; 完全快速/缓慢的集成仍然保留.

这些是源头和孤立的行为改进。
标为完全接受,同时整合、管理或目标平台
证据不足。

## 变动库存

| 文件 | 原因 |
|---|---|
| `core/src/executor/{mod.rs,transparent.rs,post_condition.rs}`, `contracts/src/control/trace.rs` | 导出平台回溯检查器; 区分未验证状态和最终跟踪成功 |
| `adapters/linux/src/telemetry/{mod.rs,provider.rs,power.rs}`, `adapters/windows/src/telemetry/provider.rs` | 正确的网络率; 读取 Linux 供电数据 |
| `ai/src/{lib.rs,bounded.rs,intent.rs,orchestration.rs}`, `apps/aicored/src/runtime.rs`, `ai-runtime/src/{lib.rs,devices.rs}` | 锁定AI/特意呼叫,验证意图政策,协调模拟代理和路线注册模式运行时间 |
| `runtime/src/{adaptive_loop.rs,dual_loop.rs,lib.rs}`, `runtime/Cargo.toml` | 保留NoOp/同意语义并增加不可改变的政策交割部分 |
| `contracts/src/ipc/protocol.rs`, `ipc/src/{client.rs,server.rs,identity.rs,transport/linux.rs,transport/windows.rs}`, `apps/aicored/src/{cli.rs,ipc_handler.rs,main.rs,scheduler.rs}`, `apps/aicore-cli/src/main.rs`, `ipc/Cargo.toml` | IPC 谈判, OS 同行身份、每次行动授权和L1/L2申请请求 |
| `plugin/src/{activation.rs,host/process.rs,lib.rs}`, `apps/aicored/src/scheduler.rs` | 固定失效的插件激活和管理生命周期;防止重复的崩溃记录 |
| `contracts/src/{effect.rs,ai/intent.rs,hardware/matrix.rs,memory/context.rs,traits/intent_agent.rs,traits/safe_actuator.rs}`, `core/src/{effect.rs,safe_actuator.rs,policy/thermal.rs}`, `memory/src/{personal.rs,lib.rs}` | 衡量、个人背景、同意、意图和安全能力合同 |
| `tier-l1/src/task_source.rs`, `tier-l2/src/{lib.rs,native_provider.rs,register.rs}` | 保留当前任务所有权并将本地控件连接到现有队列 |
| `tests/{control.trace.unit.test.rs,transparent.control.unit.test.rs,pipeline.taskPipeline.integration.test.rs,l2.standalone.unit.test.rs}` | 更正后执行和任务语义的后退范围 |
| `docs/contracts/IPC_PROTOCOL.md`, `docs/quality/ENGINEERING_STATUS.md` | 合同和地位的一致性 |
| `docs/plugin/{PLUGIN_LIFECYCLE.md,PLUGIN_SECURITY.md}`, `docs/runtime/RUNTIME_ARCHITECTURE.md`, `docs/memory/MEMORY_ARCHITECTURE.md` | 将目的地设计与目前有线和测试的代码区分开来 |
| `docs/quality/{DESTINATION_REQUIREMENTS.md,DESTINATION_TEST_SAFETY.md,DESTINATION_VERIFICATION_REPORT.md}`, `scripts/verify/model_safe_workspace_tests.sh` | 要求矩阵、安全验证和现有证据 |

库存包括修改的文件和新文件;没有删除
源文件。七个受保护的剩余文件既没有编辑也没有编辑
更名为.

初始目的批次路径(上表和
最后的工作树差异; 这不是一个完整的改变路径列表 :

```text
adapters/linux/src/telemetry/mod.rs
adapters/linux/src/telemetry/provider.rs
adapters/linux/src/telemetry/power.rs
adapters/windows/src/telemetry/provider.rs
ai/src/lib.rs
ai/src/bounded.rs
apps/aicore-cli/src/main.rs
apps/aicored/src/ipc_handler.rs
apps/aicored/src/runtime.rs
apps/aicored/src/scheduler.rs
contracts/src/control/trace.rs
contracts/src/ipc/protocol.rs
core/src/executor/mod.rs
core/src/executor/transparent.rs
core/src/executor/post_condition.rs
docs/contracts/IPC_PROTOCOL.md
docs/quality/ENGINEERING_STATUS.md
docs/quality/DESTINATION_REQUIREMENTS.md
docs/quality/DESTINATION_TEST_SAFETY.md
docs/quality/DESTINATION_VERIFICATION_REPORT.md
ipc/src/client.rs
ipc/src/server.rs
ipc/src/transport/linux.rs
runtime/src/adaptive_loop.rs
scripts/verify/model_safe_workspace_tests.sh
tests/control.trace.unit.test.rs
tests/l2.standalone.unit.test.rs
tests/pipeline.taskPipeline.integration.test.rs
tests/transparent.control.unit.test.rs
tier-l1/src/task_source.rs
tier-l2/src/lib.rs
tier-l2/src/native_provider.rs
tier-l2/src/register.rs
```

其他新文件包括: `plugin/src/activation.rs`, `ipc/src/identity.rs`,
`runtime/src/dual_loop.rs`, `memory/src/personal.rs`, `ai/src/{intent.rs,orchestration.rs}`,
`ai-runtime/src/devices.rs`, `contracts/src/{effect.rs,ai/intent.rs,hardware/matrix.rs,traits/intent_agent.rs,traits/safe_actuator.rs}`
和 `core/src/{effect.rs,safe_actuator.rs,policy/thermal.rs}`. 删除的源文件:**无**。

## 审定证据

| 检查 | 结果 | 范围 |
|---|---|---|
| `cargo metadata --no-deps --format-version 1` | PASS, 退出码 0 | 工作空间清单 |
| `cargo check --workspace --all-targets --locked` | PASS, 退出码 0 | WSL/Linux 工作空间 |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | PASS, 退出码 0 | WSL/Linux 工作空间 |
| `cargo fmt --all -- --check` | PASS, 退出码 0 | 工作空间格式化 |
| `bash scripts/verify/model_safe_workspace_tests.sh` | PASS, 退出码 0: 137 测试目标; 666通过, 0 失败, 0 忽略, 43 过滤 | 工作空间测试的安全子集;见`DESTINATION_TEST_SAFETY.md` |
| `cargo test --workspace --all-targets --locked` 无过滤器 | SKIPPED (英语). | 包含被禁止的现场控制/现场主机测试 |
| `TMPDIR=/tmp/aicore-destination2-smoke AICORE_EXECUTION_MODE=model cargo run -p aicore-daemon --bin aicored --locked -- --once --config /tmp/aicore-destination2-smoke/config.json --state-path /tmp/aicore-destination2-smoke/status.json --ipc-path /tmp/aicore-destination2-smoke/aicored.sock --memory-path /tmp/aicore-destination2-smoke/memory.db --plugins-dir /tmp/aicore-destination2-smoke/plugins --log-format json` | PASS,退出码 0: 1个模拟控制周期、6个模型结果、1个装入 JSON 模型 | 路由是本地的且绑定的, 没有报告后端动作; 这不是硬件成功或完整的 AI 路由证据 |
| `modelNORealPC.execution.unit.test.rs` 安全工作空间运行 | PASS (英语). | 模拟/模型 AI CPU 路由、绑定器、任务、内存和事件链 |
| `cargo check -p aicore-ipc -p aicore-adapter-windows -p aicore-tier-l2 -p aicore-core --all-targets --locked --target x86_64-pc-windows-msvc`和`cargo check -p aicore-plugin -p aicore-ai -p aicore-ai-runtime --all-targets --locked --target x86_64-pc-windows-msvc` 数据 | PASS, 退出码 0 | 仅定向交叉编译;无 Windows 运行时间 |
| `cargo check -p aicore-daemon --all-targets --locked --target x86_64-pc-windows-msvc` | FAIL,退出码 1:捆绑 SQLite 无法找到构建依赖 MSVC `lib.exe` 输入 WSL | 工具链/ 环境限制; 守护进程 Windows 目标在此没有代码验证 |
| `bash scripts/audit_v4_static.sh` | PASS, 退出码 0: 110 Rust 测试文件已检查 | 静态无变体 |
| `bash scripts/audit_v32_static.sh` | PASS, 退出码 0 | 历史静态结构检查 |
| `git ls-files -z '*.sh' \| xargs -0 -n1 bash -n`，并检查新安全脚本 | PASS, 退出码 0 | 命令行语法 |

继续记录的文件库存
`/tmp/aicore-destination2-files-before.txt` 并将其与
`/tmp/aicore-destination2-files-after.txt`。没有先前存在的非`target/`
7个被明确保护的剩余文件相同
前后时间 SHA256个散列。 `target/`;
这些瞬息万变的构建制品被排除在源文件的完整性之外。

早先的 `V4_DEFECT_CORRECTION_REPORT.md` 记录了不同的验证
语句;其未过滤的666测试结果不作为当前证据。

## 剩余目的地工作和特定阻塞器

- **MVP 效果/反馈:** 数据结构和保守比较逻辑
  已存在,但现场成对的延迟/通量/能量收集和反馈
  模式控制周期无法建立好处。
- ** 插件生命周期:** 固定的通用扩展激活有模拟覆盖,
  但输入式遥测/控制器能力装订和全部崩溃/启动
  Windows 进程权限需要本地验证。
- ** 申请身份和 IPC:** 不变。UID Linux 同行和流程寿命
  存在身份约束。应用签名、SID 与令牌证明，以及失效注册信息清理
  清理,原生管道 ACL行为和兼容升级仍需工作.
- ** 快速/慢控制:** 新的交接测试证明是一种有限度的*组件*,
  不是守护进程控制截止日期。 遥测、 历史、 预测和运行时间
  绑定在 `AdaptiveLoop::tick` 中保持同步; 延迟绑定仍然可能
  延迟控制。 将组件与 SafeMode 合并, 调度器是
  中央密码阻断器
- ** V2:** 抽象意图代理人和设备目录没有实际本地
  LLM 提供者/模型、守护进程意图进入点或 GPU/NPU 模式运行时间 。
  小时模式预测没有预期的准确性/效益数据集。
- **V3:** 矩阵和被监视的动因抽象语没有来自
  支持的设备或连接到平台绑定器。可选热
  守护进程无法启用。 高级调度和测量功率
  优化仍然缺失。 内核整合仍为有条件 。
- ** 释放/跨平台:** Windows 本地守护进程/ 服务行为, Linux
  包件安装、多厂商硬件、长期间接费用和热安全
  目标Windows编译不是全工作空间或
  运行时间认证 。

## 最终真机阶段, 未在此执行

1. 在控件上 Windows 和 Linux 主机,运行全部43个过滤测试和
   在检查每个测试的副作用后, 完成未过滤的工作空间套件 。
2. 将遥测与所支持的独立读数进行比较 CPU, GPU, NPU,
   储存、网络、电池和热设备; 记录无法使用的案例。
3. 操作过程/功率控制,在返回、性能和
   能量效应、许可拒绝、回滚和SafeMode在回收
   程序。
4. 校验 Windows 命名管道 ACL 身份、服务周期和包件
   在可支配主机中安装; 验证 Linux 包/服务寿命周期
   同样,在这项工作中也没有运行安装器或服务动作。
5. 测量持续快速启动的最后期限, 缓慢启动的后退, 闲置的间接费用,
   预测校准和用户工作负荷对验收的好处
   标记目的地完成前的模型。

** 决定:** 目的地完成**NO / NOT VERIFIED**. 保险箱
工作空间码门通过当前更改, 但列出的产品
要求和控制机器的接收仍然开放。

