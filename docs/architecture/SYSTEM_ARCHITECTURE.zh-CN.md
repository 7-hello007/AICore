[英文](SYSTEM_ARCHITECTURE.md) | [中文](SYSTEM_ARCHITECTURE.zh-CN.md)

# 系统架构

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

## 不变的依赖方向

`Application/Tier → Contracts → Core → Runtime → Adapter → OS/Hardware`。

核心层永远不直接依赖 Windows、Linux、厂商实现或语言模型。

## AI 控制链

AICore 遵循以下控制流程：

观察 → 理解 → 预测 → 决策 → 校验 → 执行 → 测量 → 学习

控制链将观测到的系统状态转化为经过校验的执行决策。
