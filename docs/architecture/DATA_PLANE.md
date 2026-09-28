[English](DATA_PLANE.md) | [Chinese](DATA_PLANE.zh-CN.md)
# Data Plane

```mermaid
flowchart LR
    OS[OS + Hardware]
    TEL[Telemetry]
    NORM[Normalizer]
    STATE[Computer State]
    EVENTS[Event Bus]
    DASH[Dashboard]
    MEM[Historical Memory]

    OS --> TEL --> NORM --> STATE
    STATE --> EVENTS
    EVENTS --> DASH
    STATE --> MEM
    MEM --> DASH
```

The data plane must ensure that:

- Terminal and web dashboards use the same source.
- Real-time state remains separate from historical data.
- UI code does not access the OS directly.
- High-frequency data collection does not block the control plane.
