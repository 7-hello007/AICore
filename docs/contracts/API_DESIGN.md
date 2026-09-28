[English](API_DESIGN.md) | [Chinese](API_DESIGN.zh-CN.md)
# API Design

## API Categories

| Type | Purpose |
|---|---|
| Public API | External applications and CLI |
| Internal API | Modules within Core and Runtime |
| Platform Contract | Windows and Linux adapters |
| AI Runtime Contract | Models and inference backends |
| Tier Contract | L0/L1/L2 integration |
| Plugin API | Dynamically loaded plugins |

## Version Policy

- Additive changes increment the minor version.
- Backward-incompatible changes increment the major version.
- Deprecated APIs require migration guidance.
- Internal changes must preserve the stable public API.