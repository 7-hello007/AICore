[英文](ADAPTER_RULES.md) | [中文](ADAPTER_RULES.zh-CN.md)

# 适配器规则

1. 适配器不创建策略。
2. 适配器不持久化用户画像。
3. 适配器不读取用户界面。
4. 适配器只能通过契约返回状态、能力或执行结果。
5. 不支持的能力必须返回明确 `NotSupported/CapabilityUnavailable`，不能伪装成成功。
6. 每个适配器必须通过同一套契约测试。
