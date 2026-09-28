[English](PERMISSION_MODEL.md) | [Chinese](PERMISSION_MODEL.zh-CN.md)
# Permission Model

Proposed capabilities are divided into:

- telemetry.read
- process.read
- process.control
- power.control
- gpu.read
- gpu.control
- npu.read
- npu.control
- task.submit
- plugin.install
- plugin.control

Use least privilege by default. Check control requests at the identity, capability, and action boundaries.
