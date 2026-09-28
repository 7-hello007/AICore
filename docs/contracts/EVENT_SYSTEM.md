[English](EVENT_SYSTEM.md) | [Chinese](EVENT_SYSTEM.zh-CN.md)
# Event System

Events describe facts; they do not execute policies.

Core events:

- `SystemStateUpdated`
- `ContextUpdated`
- `TaskSubmitted`
- `PolicyUpdated`
- `ExecutionCompleted`
- `RuntimeModeChanged`
- `AiSuggestionCreated`

Event Bus must support:

- collect publication outcomes
- unsubscribe
- bounded buffering/backpressure
- async subscribers
- failure isolation
