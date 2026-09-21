# Event System

事件只描述事实，不执行策略。

核心事件：

- `SystemStateUpdated`
- `ContextUpdated`
- `TaskSubmitted`
- `PolicyUpdated`
- `ExecutionCompleted`
- `RuntimeModeChanged`
- `AiSuggestionCreated`

Event Bus 必须支持：

- publish result collection
- unsubscribe
- bounded buffering/backpressure
- async subscribers
- failure isolation
