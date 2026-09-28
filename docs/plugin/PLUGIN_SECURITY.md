[English](PLUGIN_SECURITY.md) | [Chinese](PLUGIN_SECURITY.zh-CN.md)
# Plugin Security

The plugin boundary must support:

- artifact digest
- Ed25519 signature
- key id / trust store
- explicit permissions
- process isolation where required
- resource budget validation (`memory_limit_mb > 0`); current subprocess host does **not** enforce an OS memory limit
- timeout enforcement
- failure counter
- disable / rollback

Local-development bypasses are not production trust controls.

OS-enforced memory limits require a platform-specific implementation and separate validation. The current
`PluginProcessSpec.memory_limit_mb` field validates a positive budget but does not enforce a process memory
limit. Standard-output and standard-error readers limit a single line to 64 KiB; an oversized standard-output
line terminates the plugin and records a crash.

The activator rejects unpinned plugins, artifact or manifest hash mismatches, invalid signatures, permissions not
present in the pin, and unauthorized typed bindings. `process.priority_v1` can bind only one active Adapter
plugin with `process.control` permission and an operator pin containing `allow_control=true`. A plugin ACK does
not prove that control took effect; the platform process adapter reads priority independently, and a failed
readback enters Verify/Rollback.

PID 0 is rejected before a request reaches the plugin, preventing implicit selection of either the plugin process
or the current process. GPU, NPU, and power plugin controls are not bound. `BuiltIn` trust can only come from an
operator pin; a manifest cannot elevate its own trust. On Linux, the daemon requires the policy file to be owned
by the running UID and not writable by group or other users. `memory_limit_mb` remains budget validation rather
than an OS resource quota. Model Mode never starts a plugin subprocess.
