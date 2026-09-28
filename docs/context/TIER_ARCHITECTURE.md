[English](TIER_ARCHITECTURE.md) | [Chinese](TIER_ARCHITECTURE.zh-CN.md)
# Tier Architecture

## Life cycle

`Discover → Validate → Register → Active → Degraded → Disabled → Removed`

## Registry Rules

The registry manages discovery, capabilities, versions, and lifecycle state. It does not make policy decisions.

Each Tier must declare:

- `tier_id`
- `version`
- `capabilities`
- `dependencies` (only Core contracts or declared capabilities)
- `permissions`

## Removal rules

- Removing L0 does not require changes to L1 or L2.
- Removing L1 does not require changes to L0 or L2.
- Removing L2 does not require changes to L0 or L1.
- Core business logic must not branch on `TierLevel` to support a specific tier.

## Three-Tier Independence

AICore provides three independent compatibility tiers:

- L0 — Real-world integration tier
- L1 — Controlled integration tier
- L2 — Deep integration tier

L0, L1, and L2 remain independent at the architectural level.
Core Contracts provide the stable integration boundary between the tiers.

The tiers do not directly depend on the implementation of the other tiers.
