# Model Lifecycle

`Discover → Validate → Load → Warm → Infer → Monitor → Degrade/Fallback → Unload`

每个模型必须有：

- name/version
- model kind
- runtime kind
- input schema
- output schema
- latency budget
- privacy class
- capabilities

模型升级必须支持 rollback。
