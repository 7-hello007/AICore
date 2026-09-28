[English](SYSTEM_BOUNDARY.md) | [Chinese](SYSTEM_BOUNDARY.zh-CN.md)
# System Boundary

```mermaid
flowchart TB
    APP[Applications]
    TIER[L0 / L1 / L2 Independent Compatibility Adapters]
    CORE[Core Contracts + Context + Policy + Memory]
    RUNTIME[Runtime + Validator + Executor + Scheduler]
    ADAPTER[OS / Hardware / AI Runtime Adapters]
    OS[Windows / Linux]
    HW[CPU / GPU / NPU / Memory / Power]

    APP --> TIER
    TIER --> CORE
    CORE --> RUNTIME
    RUNTIME --> ADAPTER
    ADAPTER --> OS
    OS --> HW
```

## AICore owns

- Context and workload inference
- Policy creation and resolution
- Validation
- Runtime orchestration
- Compatibility adapters
- Historical memory and policy effectiveness
- IPC / Plugin control plane
- Dashboard data plane

## AICore does not own

- Application business logic
- GPU/NPU firmware
- Vendor driver implementation
- Kernel implementation unless future integration requires it
- Model training infrastructure
