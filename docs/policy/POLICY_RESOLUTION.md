# Policy Resolution

冲突解析的默认优先级：

```text
Safety
> Explicit User Intent
> Security / Privacy
> Interactive Latency
> Deadline
> Performance
> Energy
> Background Optimization
```

任何 Policy 组合必须产生可解释的 resolution record：

- input policies
- conflict set
- chosen winner
- reason
- validator result
