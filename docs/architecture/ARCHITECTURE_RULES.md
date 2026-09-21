# Architecture Rules

| Rule | Requirement |
|---|---|
| R-01 | Core 不依赖 Platform |
| R-02 | Core 不依赖 UI |
| R-03 | Core 不依赖具体 AI Model |
| R-04 | Tier 之间不互相依赖 |
| R-05 | Adapter 只实现接口，不承载业务决策 |
| R-06 | AI 不直接执行系统操作 |
| R-07 | 所有策略必须经过 Validator |
| R-08 | 所有系统控制必须经 Executor |
| R-09 | 所有跨模块状态通过 Contract/Event/DTO |
| R-10 | 新功能优先增加模块/插件，而不是修改 Core |
| R-11 | 运行时失败必须可回退 |
| R-12 | 删除任一 Tier/Plugin 不应破坏其他 Tier/Plugin |
