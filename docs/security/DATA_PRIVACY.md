# Data Privacy

默认原则：Local-first、Data-minimization、Explicit retention。

模型输入前必须检查：

- 是否包含用户敏感数据
- 是否允许进入该模型
- 是否允许云端
- 是否能匿名化

所有历史数据都必须支持 retention/deletion/audit。

当前 `HistoricalRecord.application_id` 只来自显式注册的 L1/L2/队列任务；
`history_enabled` 为 false 时不写入，`profile_enabled` 为 false 时不读取用于
个性化。应用 ID 仍属于可关联的个人行为数据，随历史记录接受同一 SQLite/JSON
retention、清空和审计流程。聚合模式至少需要三个同一应用、同一 UTC 小时样本；
预测时必须有对应的显式任务身份，不能用窗口标题推断身份。
