[English](SYSTEM_REQUIREMENTS.md) | [Chinese](SYSTEM_REQUIREMENTS.zh-CN.md)
# System Requirements

## Functional requirements

| ID | Requirement |
|---|---|
| FR-01 | Collect CPU, GPU, NPU, memory, power, process, and application state |
| FR-02 | Normalize platform data into shared contracts |
| FR-03 | Identify workload and interaction context |
| FR-04 | Generate and validate resource policy |
| FR-05 | Apply permitted resource policy through adapters or runtimes |
| FR-06 | Retain authorized history |
| FR-07 | Support the three independent L0, L1, and L2 compatibility paths |
| FR-08 | Provide IPC, CLI, terminal dashboard, and web dashboard interfaces |
| FR-09 | Support plugin discovery, trust checks, lifecycle management, and isolation |
| FR-10 | Support AI runtime selection, fallback, and model lifecycle management |

## Non-functional requirements

| Category | Requirement |
|---|---|
| Safety | AI cannot bypass the Validator or Executor to modify the system directly |
| Isolation | Any tier, plugin, or AI runtime can be disabled independently |
| Portability | Windows and Linux share Core; platform differences remain in adapters |
| Performance | AI and dashboard overhead must be measurable and bounded |
| Observability | Decisions, policies, executions, and failures are traceable |
| Compatibility | Existing software works through L0 without mandatory changes |
| Testability | Public interfaces, adapters, pipelines, and failure paths are testable |
