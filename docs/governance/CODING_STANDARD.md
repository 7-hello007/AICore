[English](CODING_STANDARD.md) | [Chinese](CODING_STANDARD.zh-CN.md)
# Coding Standard

Core requirements:

- Rust first for system service
- no business logic in adapters
- no OS checks in Core
- errors use canonical error model
- avoid panic in production paths
- explicit ownership of background tasks
- deterministic tests where possible
- all unsafe blocks documented and isolated
- no dead code kept for old architecture unless explicitly marked compatibility
