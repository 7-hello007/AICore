[英文](MODEL_LIFECYCLE.md) | [中文](MODEL_LIFECYCLE.zh-CN.md)

# 模型生命周期

`Discover → Validate → Load → Warm → Infer → Monitor → Degrade/Fallback → Unload`

每个模型必须有：

- 名称与版本
- 模型类别
- 运行时类别
- 输入模式
- 输出模式
- 延迟预算
- 隐私等级
- 所需能力

模型升级必须支持回滚。
