# Product Definition

## 产品定位

AICore 是一个 **AI Computer Control Layer**：位于应用软件与操作系统/硬件之间，通过上下文理解、预测、策略与资源编排，让现有计算机逐步成为 AI-managed Computer。

## 不是什么

| 不属于 AICore | 原因 |
|---|---|
| 新操作系统 | AICore 依赖并增强既有 OS，不替代 OS 本体 |
| 普通 PC 优化工具 | 核心不是固定规则清理或加速，而是上下文感知与闭环控制 |
| Chatbot | AI 的主要职责是系统理解与策略，不是聊天 |
| 硬件驱动集合 | 驱动与厂商 SDK 属于 Adapter/Runtime 边界 |
| 强制要求应用接入的 SDK | L0 兼容路径必须在应用不改代码时成立 |

## 产品价值

1. 让用户无需手工维护大量性能/功耗/后台策略。
2. 让传统软件在不改代码的情况下获得系统级 AI 适配。
3. 允许愿意参与的应用逐步获得更精细的 AICore 能力。
4. 为不同 OS、CPU/GPU/NPU、AI runtime 建立统一控制抽象。
