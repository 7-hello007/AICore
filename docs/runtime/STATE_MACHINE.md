# State Machine

## Task

`Created → Detected → Classified → Planned → Validated → Queued → Running → Completed`

异常路径：

`Running → Failed → Recovering → Degraded`

取消路径：

`Created/Queued/Running → Cancelled`

## Plugin

`Discovered → Validated → Active → Failed → Disabled/Restarting`

## Runtime

`Normal ↔ Degraded ↔ SafeMode ↔ Recovery → Normal`
