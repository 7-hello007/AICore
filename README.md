# AICore — AI Computer Control Layer

AICore 是面向 Windows 和 Linux 的 AI 原生计算机控制层。它位于应用与操作系统资源接口之间，结合平台状态、应用任务、历史信息和可选的本地 AI 推理，提出资源策略，并在能力与权限允许时执行、验证和记录结果。项目目标是让计算机能够根据上下文协调自身资源，而不是提供一组固定的“优化”开关。

根目录的 [destination.md](destination.md) 描述产品目标及阶段性设计。本 README 是整个项目的入口；目标设计、已有代码和已通过的平台验收是不同的概念。当前源码包含可运行的模型模式和模拟平台、Windows/Linux 适配代码及多项闭环组件，但不表示所有硬件、操作系统和最终产品目标已经认证。

## 系统如何工作

```text
平台能力与遥测 → Context / Workload → TierHub / Task
                → Prediction / AI proposal → Policy → Validation
                → Runtime routing / binding → Execution → Readback / Verification
                → Rollback / SafeMode → Memory / Feedback / Events / Dashboard
```

Core 通过统一契约处理上下文、策略、验证与执行结果；平台能力由运行时探测，平台差异由 Adapter 和相应 transport/provider 实现隔离。AI 可以提出预测或意图，但不能绕过策略验证。快控制侧消费有时效的策略，慢侧执行遥测、历史读取和推理；过期、失败或不可用的结果不能被当作成功执行。执行状态区分实际成功、跳过、不支持、失败与无操作，模型或模拟执行也不等同于真实硬件效果。

应用接入有三个独立等级：L0 处理无需改造的应用，L1 接收应用声明的任务信息，L2 使用原生任务接口。IPC 提供本地、版本化的请求通道；插件通过信任检查、能力绑定和受管进程生命周期接入。个人记忆采用 opt-in 和保留策略，相关预测与反馈依赖可验证的数据来源。

## 代码结构

| 目录 | 职责 |
|---|---|
| `contracts/` | 跨组件的数据结构、状态、错误及 trait 契约 |
| `core/` | Context、策略、验证、执行、读回与安全控制逻辑 |
| `runtime/`, `runtime-router/` | 控制循环、策略交接、运行时路由与绑定 |
| `adapters/windows/`, `adapters/linux/` | 操作系统遥测、能力探测与平台执行适配 |
| `adapters/simulated-platform/` | 使用采集 fixture 的内存模拟平台；用于隔离验证 |
| `ai/`, `ai-runtime/`, `models/` | 预测与意图 provider、本地推理 runtime、模型资产 |
| `api/`, `ipc/` | 应用注册/身份策略、对外契约和本地通信 |
| `tier/`, `tier-l0/`, `tier-l1/`, `tier-l2/`, `task-runtime/` | 兼容等级、任务来源、队列与生命周期 |
| `memory/` | 历史、个人记忆与持久化 |
| `plugin/` | 插件发现、信任、激活、进程隔离及能力绑定 |
| `service-host/` | 服务宿主相关代码；不要求开发时安装服务 |
| `apps/` | daemon、CLI、终端/Web Dashboard、benchmark 等应用 |
| `tests/`, `scripts/`, `examples/` | 跨 crate 测试、验证脚本与示例 |
| `deploy/`, `docs/` | 交付资源与项目级架构、接口、验收文档 |

Rust workspace 成员及准确路径以 [Cargo.toml](Cargo.toml) 为准。

## 构建与安全验证

项目使用 Rust，声明的最低版本为 1.75；工作区使用 `Cargo.lock`。在项目根目录可运行：

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
```

`cargo test --workspace --all-targets --locked` 覆盖项目各层，但部分平台测试可能调用真实进程或电源控制接口。运行完整测试前应按 [测试安全清单](docs/quality/DESTINATION_TEST_SAFETY.md) 审查环境和测试副作用；隔离测试的过滤结果须如实报告，不能写成未筛选全套通过。

daemon 通过 `AICORE_EXECUTION_MODE` 选择运行方式：

| 值 | 含义 |
|---|---|
| `model` | 本地模型与模型执行路径；不代表真实硬件控制 |
| `simulated-platform` | 采集 fixture 驱动、仅写内存的平台模拟层 |
| `platform` | 平台适配路径；可能触及宿主机资源控制，需按目标机器单独审查与授权 |

在 Windows 桌面环境，构建 daemon 和 CLI 后，可使用 `scripts/verify/simulated_platform_windows_smoke.ps1` 验证模拟 daemon tick 与 named-pipe 状态查询。该脚本只验证模拟路径，不能用于声称原生控制认证。模型分类器的本地 JSON 资产为 `models/policy_classifier_v1.json`；意图接口可使用本机 Ollama，模型名由 `AICORE_OLLAMA_MODEL` 配置（默认 `llama3.2:3b`），服务不可用时应明确报告失败或使用隔离 mock，不把 mock 结果记为真实模型调用。

## 实现与验收状态

仓库已经包含策略与执行闭环、回滚/SafeMode、任务与应用接入、插件和 IPC 骨架、个人记忆、Dashboard、本地模型及模拟平台。不同功能的完成程度并不相同：设备与 OS 覆盖、真实效果测量、跨用户 IPC 拒绝、生产插件硬件读回、高级调度和跨设备推理等仍有实现或平台验收缺口。真实 GPU/NPU/电源控制、服务安装及跨硬件认证不能从单元测试、mock 或模拟平台结果推断。

按目标逐项查看 [要求—现状—缺口—验收清单](docs/quality/DESTINATION_REQUIREMENTS.md)；测试命令、实测证据和限制见 [验证报告](docs/quality/DESTINATION_VERIFICATION_REPORT.md)。长期架构与接口文档从 [docs/README.md](docs/README.md) 开始。历史阶段报告保留在 `docs/quality/`，不作为整个项目的完成声明。

## License

Apache License 2.0；见 [LICENSE](LICENSE) 和 [NOTICE](NOTICE)。
