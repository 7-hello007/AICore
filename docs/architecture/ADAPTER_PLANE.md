[English](ADAPTER_PLANE.md) | [Chinese](ADAPTER_PLANE.zh-CN.md)
# Adapter Plane

The adapter plane is the only layer that contains platform- and hardware-specific behavior.

```mermaid
flowchart TB
    CONTRACT[IHardware / IOS / IRuntime Contracts]
    WIN[Windows Adapter]
    LIN[Linux Adapter]
    NVIDIA[NVIDIA Adapter]
    AMD[AMD Adapter]
    INTEL[Intel Adapter]
    NPU[NPU Runtime Adapter]

    CONTRACT --> WIN
    CONTRACT --> LIN
    CONTRACT --> NVIDIA
    CONTRACT --> AMD
    CONTRACT --> INTEL
    CONTRACT --> NPU
```

Adapters do not define policy, retain history, or own UI state.
