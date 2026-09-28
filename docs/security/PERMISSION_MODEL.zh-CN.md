[英文](PERMISSION_MODEL.md) | [中文](PERMISSION_MODEL.zh-CN.md)

# 权限模型

权限建议分为：

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

默认采用最小权限；控制能力必须经过能力、权限和策略三重检查。
