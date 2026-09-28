[英文](ADAPTER_PLANE.md) | [中文](ADAPTER_PLANE.zh-CN.md)

# 适配器平面

适配器是平台与硬件差异的唯一归属层。

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

适配器不包含策略、不负责历史数据，也不持有界面状态。
