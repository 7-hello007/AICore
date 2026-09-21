# Resource Control

资源控制分三类：

| Capability | 说明 |
|---|---|
| Observe | 只读状态 |
| Recommend | 生成建议 |
| Control | 允许真实执行 |

支持资源：CPU、GPU、NPU、Memory、Power、Process。

任何 Control 必须先检查：Capability、Permission、Policy、Runtime availability。
