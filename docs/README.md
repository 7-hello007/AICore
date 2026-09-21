# AICore Global Documentation

## 目的

本目录是 AICore 整个产品生命周期的**唯一长期架构与工程文档体系**。

它不按 V1/V2/V3/V4/V5 拆分。版本只描述产品能力的演进；本目录描述的是系统始终遵守的架构、接口、数据、运行、安全、测试与交付规则。

当前项目文档只描述稳定架构、接口、工程规则与可验证事实；未来版本路线不作为产品源码依赖。

## 核心原则

1. Core 不依赖具体 OS、硬件厂商、AI 模型或 UI。
2. 所有平台差异通过 Adapter / Runtime Provider 隔离。
3. L0 / L1 / L2 三种兼容等级彼此独立，只依赖 Core Contracts，不互相调用。
4. AI 负责理解、预测和提出策略；Validator 与 Executor 决定是否以及如何执行。
5. Fast Loop 负责确定性执行，Slow Loop 负责智能决策。
6. 失败必须可降级，AICore 失败不能拖垮操作系统。
7. 所有公共接口、事件、数据结构和 IPC 协议必须可测试、可版本化、可追踪。

## 当前状态

本目录描述的是**目标架构与工程规则**。当前代码是否已经符合这些规则，以 `docs/quality/ENGINEERING_STATUS.md` 与 CI/平台验收结果为准。

## 文档结构

```text
core/               系统核心架构与边界
contracts/          契约/API/IPC/事件/插件接口
context/            Context、Workload、Compatibility
ai/                 AI runtime、模型生命周期、推理策略
policy/             Policy、资源控制与冲突解决
runtime/            Runtime、执行、状态机、失败恢复
platform/           Windows/Linux 与硬件适配
memory/             Memory、历史数据、隐私
plugin/             Plugin 生命周期、信任与隔离
ui/                 Terminal + Web Dashboard
security/           安全、权限、数据隐私
operations/         配置、日志、指标、CI/CD、发布
quality/             测试、Benchmark、Traceability、验收
architecture/       全局 Mermaid 架构图与 ADR 索引
governance/         文档、代码和接口治理
```
