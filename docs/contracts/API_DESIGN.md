# API Design

## API 类别

| 类型 | 用途 |
|---|---|
| Public API | 外部应用与 CLI |
| Internal API | Core/Runtime 内部模块 |
| Platform Contract | Windows/Linux Adapter |
| AI Runtime Contract | 模型与推理后端 |
| Tier Contract | L0/L1/L2 |
| Plugin API | 动态插件 |

## 版本策略

- additive change：minor
- backward-incompatible change：major
- deprecated API 必须有迁移说明
- 内部实现变化不得改变稳定 Public API
