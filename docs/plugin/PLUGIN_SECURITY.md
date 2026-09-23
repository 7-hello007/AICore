# Plugin Security

必须支持：

- artifact digest
- Ed25519 signature
- key id / trust store
- explicit permissions
- process isolation where required
- resource budget validation (`memory_limit_mb > 0`); current subprocess host does **not** enforce an OS memory limit
- timeout enforcement
- failure counter
- disable / rollback

Local-development bypass 不得等同于 production trust。

OS 级内存限制需要单独的平台隔离实现和验证；不得把 `PluginProcessSpec.memory_limit_mb` 的参数校验当作已经限制了插件进程内存。子进程 stdout/stderr 单行读取上限为 64 KiB；超限 stdout 导致该插件终止并记录 crash。
