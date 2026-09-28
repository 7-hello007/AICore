[English](PLUGIN_ARCHITECTURE.md) | [Chinese](PLUGIN_ARCHITECTURE.zh-CN.md)
# Plugin Architecture

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

A plugin is a black box; Core accesses it only through the plugin API and declared capabilities.
