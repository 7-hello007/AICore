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

当前激活器拒绝未 pin 的插件、hash/签名不匹配的 artifact、未列入 pin 的
权限，以及所有尚未有 typed binding 的控制权限。BuiltIn 等级只能由
操作员 pin 赋予；manifest 不能自行升级 trust。Linux daemon 要求 policy
文件由运行 UID 拥有且不能被 group/other 写。`memory_limit_mb` 仍仅校验，
不是 OS 级资源配额。Model Mode 不执行插件子进程。
