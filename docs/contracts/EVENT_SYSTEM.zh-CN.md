[英文](EVENT_SYSTEM.md) | [中文](EVENT_SYSTEM.zh-CN.md)

# 事件系统

事件只描述事实，不执行策略。

核心事件：

- `SystemStateUpdated`
- `ContextUpdated`
- `TaskSubmitted`
- `PolicyUpdated`
- `ExecutionCompleted`
- `RuntimeModeChanged`
- `AiSuggestionCreated`

事件总线必须支持：

- 发布结果收集
- 取消订阅
- 有界缓冲与背压
- 异步订阅者
- 故障隔离
