# Permission Model

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

默认最小权限；控制能力必须经过 capability + permission + policy 三重检查。
