[English](OBSERVABILITY.md) | [Chinese](OBSERVABILITY.zh-CN.md)
# Observability

Three layers:

| Layer | Contents |
|---|---|
| Logs | lifecycle, errors, decisions |
| Metrics | latency, failures, resource overhead |
| Traces | tick/control/IPC/plugin execution |

Every significant decision must include `trace_id`, `policy_id`, and `task_id` when available.
