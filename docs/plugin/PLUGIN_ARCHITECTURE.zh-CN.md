[英文](PLUGIN_ARCHITECTURE.md) | [中文](PLUGIN_ARCHITECTURE.zh-CN.md)

# 插件架构

```mermaid
flowchart LR
    DISCOVER[Discover]
    MANIFEST[Manifest Validate]
    TRUST[Trust Check]
    LOAD[Load/Spawn]
    ACTIVE[Active]
    FAIL[Failure]
    DISABLE[Disabled]

    DISCOVER --> MANIFEST --> TRUST --> LOAD --> ACTIVE
    ACTIVE --> FAIL --> DISABLE
```

插件是黑盒；核心层只能访问插件 API 和已声明的能力。
