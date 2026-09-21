# System Requirements

## 功能需求

| ID | Requirement |
|---|---|
| FR-01 | 采集 CPU/GPU/NPU/Memory/Power/Process/Application 等系统状态 |
| FR-02 | 将平台数据归一化为统一 Computer State |
| FR-03 | 识别 Workload 与 Interaction Context |
| FR-04 | 生成并验证 Resource Policy |
| FR-05 | 通过 Adapter/Runtime 执行可允许的资源策略 |
| FR-06 | 保存历史 Workload、Policy、Result 与 Profile |
| FR-07 | 支持 L0/L1/L2 三种独立兼容路径 |
| FR-08 | 提供 IPC、CLI、Terminal Dashboard 和 Web Dashboard |
| FR-09 | 支持 Plugin 发现、信任、生命周期和隔离 |
| FR-10 | 支持 AI runtime、Fallback 和 Model Lifecycle |

## 非功能需求

| 类别 | 要求 |
|---|---|
| Safety | AI 不能绕过 Validator/Executor 直接修改系统 |
| Isolation | 任一 Tier、Plugin、AI Runtime 可单独禁用 |
| Portability | Windows/Linux 共享 Core，平台差异进入 Adapter |
| Performance | AI 与 Dashboard 本身必须有可测量的 overhead budget |
| Observability | Decision、Policy、Execution、Failure 可追踪 |
| Compatibility | 现有软件可以通过 L0 工作，不强制修改应用 |
| Testability | 公共接口、Adapter、Pipeline、故障路径均可测试 |
