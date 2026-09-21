# Tier Architecture

## 生命周期

`Discover → Validate → Register → Active → Degraded → Disabled → Removed`

## Registry 规则

Registry 只负责发现、能力、版本和生命周期，不参与业务决策。

每个 Tier 必须声明：

- `tier_id`
- `version`
- `capabilities`
- `dependencies`（只能指向 Core Contracts / declared capabilities）
- `permissions`

## 删除规则

- 删除 L0 不得修改 L1/L2。
- 删除 L1 不得修改 L0/L2。
- 删除 L2 不得修改 L0/L1。
- Core 不应出现 `match TierLevel` 形式的业务分支来支持某个具体 Tier。

## Three-Tier Independence

AICore provides three independent compatibility tiers:

- L0 — Real-world integration tier
- L1 — Controlled integration tier
- L2 — Deep integration tier

L0, L1, and L2 remain independent at the architectural level.
Core Contracts provide the stable integration boundary between the tiers.

The tiers do not directly depend on the implementation of the other tiers.
