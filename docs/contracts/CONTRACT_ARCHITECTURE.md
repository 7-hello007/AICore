# Contract Architecture

Contracts 是整个系统的长期边界。

```mermaid
flowchart LR
    DOMAIN[Domain DTO]
    API[Public API]
    IPC[IPC Contract]
    EVENT[Event Contract]
    PLUGIN[Plugin Contract]
    RESOURCE[Resource Contract]
    TIER[Tier Contract]

    DOMAIN --> API
    DOMAIN --> IPC
    DOMAIN --> EVENT
    DOMAIN --> PLUGIN
    DOMAIN --> RESOURCE
    DOMAIN --> TIER
```

## 规则

- 一个 canonical error model。
- 一个 canonical schema version model。
- 跨模块时间统一 `unix_ms`。
- 不为同一业务语义维护多个重复结构。
- Contract 改动必须有 compatibility review 和 contract tests。
