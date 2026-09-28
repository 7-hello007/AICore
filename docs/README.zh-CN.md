[英文](README.md) | [中文](README.zh-CN.md)

# AICore 文档

本目录保存产品架构、接口契约、工程规则和验证证据。[项目总览](../README.zh-CN.md)介绍项目；[目标设计](../destination.zh-CN.md)定义更广的产品设计。架构文档说明目标边界；当前实现状态以[工程状态](quality/ENGINEERING_STATUS.zh-CN.md)和[目标要求清单](quality/DESTINATION_REQUIREMENTS.zh-CN.md)为准。已完成的证据见[验证报告](quality/DESTINATION_VERIFICATION_REPORT.zh-CN.md)；未完成测试统一放在[待验收测试](quality/OUTSTANDING_TESTS.zh-CN.md)。

每份文档以英文 `.md` 为主文件，以 `.zh-CN.md` 为简体中文副文件。页面顶部可以切换语言；中文页面的内部链接优先指向中文副文件。代码、命令、模型标识和 PASS／FAIL／SKIPPED／NOT VERIFIED 等状态词在两个版本中保留原文。

## 架构原则

1. 核心层不依赖具体操作系统、硬件厂商、AI 模型或界面。
2. 平台差异由适配器和运行时提供者隔离。
3. L0、L1、L2 是通过共同契约接入的独立兼容等级。
4. AI 提出建议；策略验证和执行层决定是否实施。
5. 慢侧观测与推理把有时效的策略交给快控制侧。
6. 故障必须安全降级，不能拖垮操作系统。
7. 公共契约、事件和 IPC 必须可版本化、可测试、可追踪。

## 文档地图

| 目录 | 内容 |
|---|---|
| `architecture/` | 控制面、数据面、适配面、架构决策及内核必要性分析 |
| `contracts/` | API、数据模型、事件和 IPC |
| `context/` | 上下文、工作负载和 L0/L1/L2 兼容模型 |
| `ai/` | 提供者、推理策略、意图后端和模型生命周期 |
| `policy/`、`runtime/` | 策略、路由、执行、反馈与恢复 |
| `platform/` | Windows/Linux 适配、能力矩阵与模拟层 |
| `memory/`、`plugin/` | 保留期与隐私、插件信任与生命周期 |
| `ui/`、`security/` | 仪表盘与安全边界 |
| `operations/` | 配置、观测、CI 和交付 |
| `quality/` | 验收模型、已完成证据、待验收测试与历史阶段报告 |
| `governance/` | 代码、文档与决策规则 |

逐页入口见[文档索引](DOCUMENTATION_INDEX.zh-CN.md)。历史 V3.2/V4 报告作为带日期的证据保留，不代表整个 destination 产品已经验收。
