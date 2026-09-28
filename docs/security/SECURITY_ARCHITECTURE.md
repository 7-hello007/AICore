[English](SECURITY_ARCHITECTURE.md) | [Chinese](SECURITY_ARCHITECTURE.zh-CN.md)
# Security Architecture

```mermaid
flowchart TB
    CLIENT[Client/App]
    AUTH[Identity / Permission]
    CONTRACT[Contract Validation]
    POLICY[Policy Validation]
    EXEC[Executor]
    OS[OS]

    CLIENT --> AUTH --> CONTRACT --> POLICY --> EXEC --> OS
```

Security boundaries cover at least IPC, plugins, models, runtime, configuration, and persistent data.
