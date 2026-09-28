[英文](STATE_MACHINE.md) | [中文](STATE_MACHINE.zh-CN.md)

# 状态机

## 任务

`Created → Detected → Classified → Planned → Validated → Queued → Running → Completed`

异常路径：

`Running → Failed → Recovering → Degraded`

取消路径：

`Created/Queued/Running → Cancelled`

## 插件

`Discovered → Validated → Active → Failed → Disabled/Restarting`

## 运行时

`Normal ↔ Degraded ↔ SafeMode ↔ Recovery → Normal`
