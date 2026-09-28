[英文](DESTINATION_TEST_SAFETY.md) | [中文](DESTINATION_TEST_SAFETY.zh-CN.md)
# 目标设计任务：隔离测试边界

## 历史桌面授权（2026-09-27）

在该阶段，用户允许继续进行原生操作和系统修改，但禁止运行安装包，并要求所有下载保存在项目 `env/` 下。下述五项 Windows 测试仍从**隔离测试集**中过滤，因为它们未指定新创建的受控进程，也不能保证精确恢复。这是有意过滤后的通过结果，不是未过滤的工作区测试结果。`owned_process_probe` 是另行显式运行的原生示例：它只把自己新创建的普通优先级子进程调到较高优先级，读回验证，再恢复普通优先级、验证恢复并终止和回收该子进程。若继承的优先级不是普通级别，它会拒绝执行，以免枚举值转换造成有损恢复。该示例不修改亲和性、电源、GPU/NPU 或服务状态。结果单独记录在当时的目标设计验证报告中。

应用固定策略还可接受 `allowed_principals`：Windows 从对端令牌解析 SID，Linux 解析 UID。配置非空列表时，主体检查是制品身份和逐操作授权之外的附加条件；主体缺失则默认拒绝。跨用户访问控制认证仍需单独进行。意图后端配置见[意图后端](../ai/INTENT_BACKENDS.zh-CN.md)。下方历史章节保留先前各次运行的授权和结果，不是当前执行指令。

## Windows 桌面代码续作（2026-09-24）

当时只使用 Windows 桌面副本。经审查的 Windows 安全工作区命令为 `cargo test --workspace --all-targets --locked --offline --`，附带五个 `--skip` 过滤器：`controller_handles_bad_pid_gracefully`、`executor_runs_and_reports_outcomes`、`full_loop_runs_and_records_metrics`、`history_disabled_by_default_means_no_records` 和 `history_enabled_records_tick`。第一项即使目标 PID 预期无效，也会调用进程设置器；后四项构造真实进程或电源控制器，可能应用策略。测试本身仍被保留。该次过滤测试并非完整工作区通过；结果为 141 个测试目标、711 项通过、0 项失败、0 项忽略、5 项过滤。

`AICORE_APP_POLICY_PATH` 可指向包含操作员批准应用数组的**绝对路径**，例如：

```json
[{"application_id":"aicore.cli","artifact_sha256":"<64 hex characters>","allowed_actions":["register","propose_intent","unregister"]}]
```

守护进程启动时读取该文件。配置后，每项应用操作都要求认证 IPC 对端的可执行文件 SHA-256，以及匹配的逐操作固定策略。可执行文件缺失或无法读取、固定策略不存在、操作已撤销时均拒绝请求。此摘要只是只读文件哈希，不能证明 Windows 代码签名、SID 或跨用户访问控制。未配置此可选策略时，保留既有的进程身份绑定行为。

默认的 `AICORE_INTENT_BACKEND=ollama` 暴露语义 `ProposeIntent` IPC 请求；设置为 `disabled` 时返回 `NotSupported`。`AICORE_OLLAMA_MODEL` 默认值为 `llama3.2:3b`；本机回环主机、端口及超时可通过 `AICORE_OLLAMA_HOST`、`AICORE_OLLAMA_PORT`、`AICORE_OLLAMA_TIMEOUT_MS` 配置。提供者只连接回环地址，不会启动或安装 Ollama。`aicore intent --text "..." --ipc-path <pipe>` 先注册调用应用、请求语义提案，再注销；操作员固定策略必须授权这三项操作。响应不是可执行策略；转成策略仍须经过现有策略引擎和校验器。

`scripts/verify/simulated_platform_ollama_intent.ps1` 仅使用项目构建的模拟守护进程、本地 IPC 和预先存在的 Ollama 服务。脚本在 `env/` 下使用临时文件，只停止自己的测试守护进程，从不调用平台设置器。其结果属于本地意图集成验证，不是 Windows 硬件或跨用户访问控制认证。

该阶段的授权只涵盖模型模式、模拟提供者、隔离文件系统与 IPC 样本，以及**只读**原生遥测和能力探测。未过滤的 `cargo test --workspace --all-targets --locked` 还包含调用平台进程、亲和性和电源控制器，或启动平台守护进程的测试，可能修改宿主机配置；因此在获得单独许可前没有运行未过滤命令。测试仍保持启用。

`AICORE_EXECUTION_MODE=simulated-platform` 选择仅写内存的平台适配器和连续运行的 `PlatformTickDriver`。Linux 上可执行 `bash scripts/verify/simulated_platform_smoke.sh`，Windows 上可执行 `scripts/verify/simulated_platform_windows_smoke.ps1`，使用 `docs/platform/PLATFORM_SIMULATOR.md` 描述的目标主机采集样本进行隔离的守护进程冒烟测试。这只能证明模拟平台模式，不能证明原生硬件认证。直接构造原生控制器的测试仍需下述过滤器。

`bash scripts/verify/model_safe_workspace_tests.sh` 执行工作区隔离测试。脚本审查指定测试后，通过 Cargo 的 `--skip` 过滤器排除：

- 直接调用进程亲和性、进程控制器、电源控制器或平台执行器的测试，包括可能写入系统文件接口的 Linux GPU `release()`；
- 使用默认平台模式的守护进程及发布冒烟测试；`system_smoke` 现在使用隔离遥测且不构造控制器，因此其三项测试仍运行；
- 读取真实 Linux CPU、GPU、NPU、温度遥测及 L0 观测的测试；
- 使用实时 Linux 适配器作为来源的端到端流水线。

这组保守过滤器也排除了只读的实时硬件测试。审查源码后，可单独运行选定的适配器或能力测试目标。过滤测试必须报告实际通过、失败和过滤数量；它不能满足未过滤的工作区门槛，也不能认证所有目标主机。

隔离的 `aicored --once` 冒烟测试使用 `AICORE_EXECUTION_MODE=model`，并为配置、状态、IPC、记忆和插件设置独立临时路径；旧记忆迁移默认项还需要 `TMPDIR`。其遥测只读，策略执行器和运行时绑定器均为模型实现。它不修改普通桌面环境、服务、安装包、进程优先级、亲和性或电源方案。

可能改变进程调度、电源或 GPU 状态的平台模式绑定器和执行器测试，事前需要具体授权和回滚计划。Windows 桌面上已验证同用户原生命名管道握手、确认消息及对端身份；跨用户访问拒绝和服务行为仍未验证。

真实平台模式写入测试的批准清单必须列明稳定的测试进程 PID 与名称、每条设备路径或电源目标、拟改值、影响范围、操作前状态和回滚命令。未提供显式目标时，当前守护进程会根据实时遥测动态推导目标进程，因此该动态集合不是可预先批准的固定清单。只读原生探针或模型测试也不会自动授权后续写入操作。