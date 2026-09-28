[English](CONTRACT_ARCHITECTURE.md) | [Chinese](CONTRACT_ARCHITECTURE.zh-CN.md)
# Contract Architecture

Contracts define the system's stable long-term boundaries.

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

## Rule

- Use one canonical error model.
- Use one canonical schema-version model.
- Represent cross-module timestamps consistently as `unix_ms`.
- Do not maintain duplicate structures for the same domain concept.
- Contract changes require review and contract tests.
