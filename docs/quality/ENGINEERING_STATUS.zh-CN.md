[英文](ENGINEERING_STATUS.md) | [中文](ENGINEERING_STATUS.zh-CN.md)

# 工程状态

## 范围与判断

AICore `0.4.0` 保留 V4 缺陷修复后的代码基线，并在同一架构上推进 destination。当前桌面副本在[验证报告](DESTINATION_VERIFICATION_REPORT.zh-CN.md)记载的范围内，代码门槛为 **PASS**；destination 整体产品验收仍为 **NOT VERIFIED**。模型、模拟层与原生证据都必须按各自范围解读。

代码保证策略验证先于可能改变状态的绑定，保留任务过期与执行状态语义，并提供回滚和安全模式。现有接入还包括有边界的快慢策略交接、版本协商 IPC、应用身份检查、插件信任与类型化能力契约、用户选择加入的记忆、反馈，以及可配置的本地意图和模型提供者。平台差异留在适配器和相应提供者中。

## 证据管理

已执行命令、测量值、实际覆盖范围和恢复结果见[验证报告](DESTINATION_VERIFICATION_REPORT.zh-CN.md)。当前所有未完成的验收及原因统一保存在独立的[待验收测试](OUTSTANDING_TESTS.zh-CN.md)。历史 V4 与 destination 报告章节仍是带日期的证据，不能当作本轮新测试或完整产品认证。

本轮只处理文档和源码语言，不执行硬件控制或修改宿主机配置；验证范围为源码完整性、双语导航及安全的工作区构建与静态检查，不重跑原生控制测试。
