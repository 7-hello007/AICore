[English](AI_ARCHITECTURE.md) | [Chinese](AI_ARCHITECTURE.zh-CN.md)
# AI Architecture

```mermaid
flowchart LR
    STATE[Computer State]
    CONTEXT[Context]
    MEMORY[Memory]
    MODEL[AI Model]
    SUGGEST[Suggestion]
    POLICY[Policy Engine]
    VALIDATE[Validator]

    STATE --> MODEL
    CONTEXT --> MODEL
    MEMORY --> MODEL
    MODEL --> SUGGEST
    SUGGEST --> POLICY
    POLICY --> VALIDATE
```

AI models are replaceable providers or runtimes. Core must not depend on a specific model implementation.

## Model categories

- deterministic rules
- lightweight local feature models
- SLM/LLM
- ONNX models
- vendor-accelerated models

AI does not authorize or perform final execution.
