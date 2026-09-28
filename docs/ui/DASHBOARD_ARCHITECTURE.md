[English](DASHBOARD_ARCHITECTURE.md) | [Chinese](DASHBOARD_ARCHITECTURE.zh-CN.md)
# Dashboard Architecture

Terminal and web views are two renderers of the same data plane.

```mermaid
flowchart TB
    CORE[Computer State + Dashboard Snapshot]
    IPC[IPC / Status Service]
    TUI[Terminal Renderer]
    WEB[Web Renderer]
    CORE --> IPC
    IPC --> TUI
    IPC --> WEB
```

Neither interface should collect system state independently.
