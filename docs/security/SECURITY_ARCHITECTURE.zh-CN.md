[英文](SECURITY_ARCHITECTURE.md) | [中文](SECURITY_ARCHITECTURE.zh-CN.md)

# 安全架构

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

安全边界至少覆盖 IPC、插件、模型、运行时、配置和持久化数据。
