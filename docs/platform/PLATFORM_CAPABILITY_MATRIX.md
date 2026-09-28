[English](PLATFORM_CAPABILITY_MATRIX.md) | [Chinese](PLATFORM_CAPABILITY_MATRIX.zh-CN.md)
# Platform Capability Matrix

| Capability | Windows | Linux | Contract requirement |
|---|---|---|---|
| CPU telemetry | Required | Required | Telemetry Provider |
| Process priority | Required | Required | Process Controller |
| CPU affinity | Required | Required | Process Controller |
| Power profile | Required | Best effort with declared capability | Power Controller |
| GPU telemetry | Vendor/runtime dependent | Vendor/runtime dependent | Hardware Telemetry |
| NPU telemetry | Vendor/runtime dependent | Vendor/runtime dependent | Hardware Telemetry |
| GPU control | Capability-gated | Capability-gated | Hardware Executor |
| NPU control | Capability-gated | Capability-gated | Hardware Executor |
| Service lifecycle | Windows Service | systemd/service manager | Service Contract |
