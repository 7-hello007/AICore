# Adapter Rules

1. Adapter 不创建 Policy。
2. Adapter 不持久化 UserProfile。
3. Adapter 不读取 UI。
4. Adapter 只能通过 Contract 返回状态、能力或执行结果。
5. 不支持的能力必须返回明确 `NotSupported/CapabilityUnavailable`，不能伪装成成功。
6. 每个 Adapter 必须通过同一个 Contract Test 套件。
