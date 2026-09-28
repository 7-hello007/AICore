[English](MODEL_LIFECYCLE.md) | [Chinese](MODEL_LIFECYCLE.zh-CN.md)
# Model Lifecycle

`Discover → Validate → Load → Warm → Infer → Monitor → Degrade/Fallback → Unload`

Each model must have:

- name/version
- model kind
- runtime kind
- input schema
- output schema
- latency budget
- privacy class
- capabilities

Model upgrade must support rollback.
