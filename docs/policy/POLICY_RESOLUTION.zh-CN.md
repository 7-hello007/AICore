[英文](POLICY_RESOLUTION.md) | [中文](POLICY_RESOLUTION.zh-CN.md)

# 策略冲突处理

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

任何策略组合都必须产生可解释的冲突处理记录：

- 输入策略
- 冲突集合
- 选中的策略
- 选择原因
- 校验结果
