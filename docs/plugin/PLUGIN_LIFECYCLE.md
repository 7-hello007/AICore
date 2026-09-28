[English](PLUGIN_LIFECYCLE.md) | [Chinese](PLUGIN_LIFECYCLE.zh-CN.md)
# Plugin Lifecycle

`Discover → Manifest Validate → Trust Validate → Capability Bind → Start → Health → Stop → Update/Rollback → Remove`

The plugin lifecycle is isolated from normal control execution. A plugin crash must not crash the Core process.

## Current code state

`PluginActivationManager` activates only the single artifact that matches an operator-provided pin from
`--plugin-policy`. A pin contains an absolute artifact path, two SHA-256 checks, a trust level, optional Ed25519
signature material, allowed interfaces and permissions, and process budgets. Activation reads the artifact,
validates the manifest and artifact together, reads the artifact again, and copies the validated bytes into a
private temporary directory. The process must complete a Ping/Pong health check before it becomes Active.
Stopping or dropping the managed process removes the private copy. Model Mode and simulated-platform mode
reject external plugin activation.

The current typed `extension.execute` bindings are read-only `telemetry.snapshot_v1` and process-control
`process.priority_v1`. The telemetry interface requires the Telemetry plugin kind, `telemetry.read` permission,
and an operator pin that allows the interface. `ManagedPluginTelemetryProvider` adapts the response to the
telemetry contract and returns an error when no valid response is available.

A failed process can be reactivated from the same pinned manifest. `replace_active` validates, stages, starts,
and health-checks a candidate before taking the registry lock. It rechecks replacement conditions while holding
the lock, then stops the old process and commits the new manifest and binding. If candidate activation fails, the
old binding remains active. An operator can pin the previous version and call rollback explicitly.

The process-priority interface requires the Adapter plugin kind, `process.control` permission, and an operator
pin with `allow_control=true`. The daemon selects exactly one active matching plugin. A plugin ACK records only
that the request was accepted; the platform process adapter performs an independent readback. Failure to reach
the requested priority enters the existing Verify/Rollback path. Affinity and GPU, NPU, or power control remain
outside this plugin interface. There is no automatic update or autostart policy.

In Platform Mode, the daemon uses a typed telemetry plugin only when one has already been activated. A read
failure falls back to the platform's read-only telemetry adapter. Model Mode and simulated-platform mode prohibit
external plugin processes. Therefore the current evidence is limited to mock and code-level tests; it does not
certify a real control plugin process or Windows process isolation.
