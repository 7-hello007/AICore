[英文](TIER_ARCHITECTURE.md) | [中文](TIER_ARCHITECTURE.zh-CN.md)

# 层级架构

## 生命周期

`Discover → Validate → Register → Active → Degraded → Disabled → Removed`

## 注册表规则

注册表只负责发现、能力、版本和生命周期，不参与业务决策。

每个层级必须声明：

- `tier_id`
- `version`
- `capabilities`
- `dependencies`（只能指向核心契约或已声明能力）
- `permissions`

## 删除规则

- 删除 L0 不得修改 L1/L2。
- 删除 L1 不得修改 L0/L2。
- 删除 L2 不得修改 L0/L1。
- 核心层不应出现 `match TierLevel` 形式的业务分支来支持某个具体层级。

## 三层级独立性

AICore 提供三种相互独立的应用兼容层级：

- L0 — 传统应用透明接入层级
- L1 — 应用主动声明的受控接入层级
- L2 — 深度原生接入层级

L0、L1 和 L2 在架构上保持独立。
核心契约为各层级提供稳定的集成边界。

各层级不直接依赖其他层级的实现。
