[英文](OBSERVABILITY.md) | [中文](OBSERVABILITY.zh-CN.md)

# 可观测性

三层：

| 层级 | 内容 |
|---|---|
| 日志 | 生命周期、错误、决策 |
| 指标 | 延迟、失败、资源开销 |
| 追踪 | 控制周期、控制操作、IPC、插件执行 |

所有关键决策应包含 `trace_id`、`policy_id` 和 `task_id`（如存在）。
