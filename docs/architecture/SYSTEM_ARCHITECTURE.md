[English](SYSTEM_ARCHITECTURE.md) | [Chinese](SYSTEM_ARCHITECTURE.zh-CN.md)
# System Architecture

```mermaid
flowchart TD
    USER[User Intent]
    APP[Applications]
    T0[L0 Legacy Adapter]
    T1[L1 Aware Adapter]
    T2[L2 Native Adapter]
    C[Stable Core Contracts]
    CTX[Context + Workload Fusion]
    MEM[Memory + Personal Context]
    AI[AI Runtime + Prediction]
    POL[Policy Engine]
    VAL[Validator]
    EXE[Executor]
    RT[Runtime / OS / HW Adapters]
    OS[Operating System]
    HW[Hardware]

    APP --> T0
    APP --> T1
    APP --> T2
    T0 --> C
    T1 --> C
    T2 --> C
    USER --> CTX
    C --> CTX
    CTX --> MEM
    MEM --> AI
    AI --> POL
    CTX --> POL
    POL --> VAL
    VAL --> EXE
    EXE --> RT
    RT --> OS
    OS --> HW
    HW -. feedback .-> CTX
    EXE -. result .-> MEM
```

## Dependency direction

`Application/Tier → Contracts → Core → Runtime → Adapter → OS/Hardware`.

Core never depends directly on Windows, Linux, a hardware vendor, or an LLM.

## AI Control Chain

AICore follows the control flow:

Observe → Understand → Predict → Decide → Validate → Execute → Measure → Learn

The control chain converts observed system state into validated execution decisions.
