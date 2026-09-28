[English](PLATFORM_ARCHITECTURE.md) | [Chinese](PLATFORM_ARCHITECTURE.zh-CN.md)
# Platform Architecture

Initial platforms: Windows and Linux. macOS is outside the current product scope.

```mermaid
flowchart TB
    CORE[AICore Core]
    WIN[Windows Adapters]
    LIN[Linux Adapters]
    CORE --> WIN
    CORE --> LIN
```

Core logic is shared; platform differences belong in the adapter and runtime layers.
