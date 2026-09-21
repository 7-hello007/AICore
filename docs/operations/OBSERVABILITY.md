# Observability

三层：

| Layer | Contents |
|---|---|
| Logs | lifecycle、errors、decisions |
| Metrics | latency、failure、resource overhead |
| Traces | tick/control/IPC/plugin execution |

所有关键 Decision 必须具备 trace_id / policy_id / task_id（如存在）。
