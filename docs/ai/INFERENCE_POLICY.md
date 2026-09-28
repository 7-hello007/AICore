[English](INFERENCE_POLICY.md) | [Chinese](INFERENCE_POLICY.zh-CN.md)
# Inference Policy

| Decision | Preferred mechanism |
|---|---|
| HF system control | deterministic code |
| Workload classification | rule/ML |
| Short-horizon prediction | ML |
| User intent | SLM/LLM where needed |
| High-level planning | AI planner |
| Final execution | deterministic validator/executor |

AI inference must respect `latency_budget_ms`, privacy constraints, resource budgets, and fallback policies.
