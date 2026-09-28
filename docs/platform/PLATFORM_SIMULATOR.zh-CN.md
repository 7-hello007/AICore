[英文](PLATFORM_SIMULATOR.md) | [中文](PLATFORM_SIMULATOR.zh-CN.md)
# 平台模拟层

`AICORE_EXECUTION_MODE=simulated-platform` 使守护进程通过 `adapters/simulated-platform` 运行**平台模式**流水线。连续运行使用既有 `PlatformTickDriver`；`--once` 执行一次同步控制周期。该适配器实现共享的遥测、能力、进程、电源、GPU、NPU、硬件遥测、硬件执行及受保护执行器契约。其可变状态仅在当前进程内存中。模拟层方法不会打开宿主机进程、设备或电源控制句柄；该模式也禁用插件子进程激活。守护进程本身仍会写入状态、IPC 及可选的 SQLite 记忆文件，因此测试应使用隔离路径。

附带的 Linux 样本为 `adapters/simulated-platform/fixtures/linux_wsl_readonly.json`，SHA-256 为 `8166fe921a16b46bc035227a8f8a9603226d3f242bc04cea34556f9529e7dbdb`。它于 2026-09-24 通过只读的 Linux 遥测、能力、进程读取、电源读取和硬件遥测适配器采集，命令为 `cargo run -p aicore-adapter-simulated-platform --example capture_linux`。采集程序隐去进程名、应用与窗口数据、挂载路径和网络接口名，同时保留实测数值。`snapshot.collected_at_unix_ms` 记录采集时间。样本没有插入不存在的 GPU、NPU 或电源控制值。模拟器会拒绝畸形的能力数据、快照中不存在的合成进程标识，以及缺少对应采样设备的硬件矩阵。

Windows 样本为 `adapters/simulated-platform/fixtures/windows_readonly.json`，SHA-256 为 `de6a6903f51c49619059edf4e55e90dd7772a98ddde6c92a953ad3556ffa1cec`。它于 2026-09-24 在 Windows 桌面上由只读 `capture_windows` 示例通过 Windows 遥测、能力、硬件提供者以及进程、电源读取接口采集。样本包含 10 个同时可读取优先级和亲和性的进程，并隐去了进程名、应用名、挂载路径、网络接口名和硬件名称。采集时没有取得 GPU/NPU 采样值，也没有识别出内建电源方案，因此没有补造这些数据。记录的 PID 只是模拟器的内存键，不是实时进程控制目标。Windows 构建选用该样本；Linux 构建选用 Linux/WSL 样本。

样本中的 PID **仅用作内存键**。守护进程选取第一个已采集 PID 作为模拟前台目标，第二个作为模拟后台目标，以演练既有执行器。这只是测试场景，不表示这些进程确实处于真实前台。缺失的 PID、设备或电源方案返回能力错误或 `NotSupported`。读回和回滚访问相同的内存状态。故障注入钩子可以使一次进程写入失败，或使写入后的读回值保持旧值。

运行 `bash scripts/verify/simulated_platform_smoke.sh` 可构建并执行隔离的单次控制周期及三秒连续守护进程会话。脚本为迁移默认项、配置、状态、IPC、插件和 SQLite 创建全新临时目录，并保留产物供检查。单元测试覆盖读回、不支持的设备、样本一致性和亲和性恢复。守护进程测试覆盖慢提供者过期与截止期限、新策略应用、验证失败后的回滚，以及重复失败导致进入安全模式。

在 Windows 上，离线构建项目二进制文件后运行 `scripts/verify/simulated_platform_windows_smoke.ps1`。它只启动项目构建的模拟守护进程，使用 `env/` 下的隔离路径，经命名管道检查连续控制周期以及只读命令行 `status`、`ticks` 查询，最后只停止该测试守护进程。Windows IPC 单元测试另行检查命名管道握手、确认消息和当前对端进程身份。四项守护进程模拟测试在 Windows 上使用 Windows 样本运行。

**证据范围：**模拟层仅证明已记录 Linux/WSL 和 Windows 样本下的契约接线与状态转换。它不能认证不同用户的 Windows 访问拒绝、真实优先级与亲和性操作、电源或 GPU/NPU 写入、传感器精度、性能收益，也不能在原生绑定可能阻塞时证明硬截止期限。这些都需要单独的目标主机证据；任何改变配置的操作此前还需明确批准。