[English](STATE_MACHINE.md) | [Chinese](STATE_MACHINE.zh-CN.md)
# State Machine

## Task

`Created → Detected → Classified → Planned → Validated → Queued → Running → Completed`

Failure path:

`Running → Failed → Recovering → Degraded`

Cancellation path:

`Created/Queued/Running → Cancelled`

## Plugin

`Discovered → Validated → Active → Failed → Disabled/Restarting`

## Runtime

`Normal ↔ Degraded ↔ SafeMode ↔ Recovery → Normal`
