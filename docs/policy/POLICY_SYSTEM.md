[English](POLICY_SYSTEM.md) | [Chinese](POLICY_SYSTEM.zh-CN.md)
# Policy System

Policy is derived from context, prediction, user preferences, and optional AI suggestions.

```mermaid
flowchart LR
    CONTEXT --> BASE[Base Policy]
    PRED[Prediction] --> ADAPT[Adaptive Policy]
    USER[User Preference] --> ADAPT
    AI[AI Suggestion] --> ADAPT
    BASE --> ADAPT
    ADAPT --> VALIDATE[Validator]
    VALIDATE --> EXECUTE[Executor]
```

Policy itself does not execute system calls.
