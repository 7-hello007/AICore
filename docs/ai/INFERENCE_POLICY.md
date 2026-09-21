# Inference Policy

| Decision | Preferred mechanism |
|---|---|
| 高频系统控制 | deterministic code |
| Workload classification | rule/ML |
| Short-horizon prediction | ML |
| User intent | SLM/LLM where needed |
| High-level planning | AI planner |
| Final execution | deterministic validator/executor |

AI inference 必须受 `latency_budget_ms`、privacy、resource budget 和 fallback policy 约束。
