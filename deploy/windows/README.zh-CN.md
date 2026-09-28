[英文](README.md) | [中文](README.zh-CN.md)

# AICore Windows 部署

**状态：** 实现候选；必须在 Windows 持续集成环境或目标主机上验证。

## 运行方式

- 开发模式：使用普通启动和停止脚本直接运行 `aicored.exe`。
- Windows Service：使用 `service-install.ps1` 和对应的卸载脚本注册并管理服务。

Windows Service 路径已经实现。生产部署仍应使用专用低权限服务账户，并验证访问控制、自动重启、升级与回滚行为，以及 Named Pipe 权限。

## 发布门槛

只有在原生 Windows 主机上通过服务安装、启动、查询、停止、卸载、IPC、Dashboard，以及升级与回滚冒烟测试后，才能将该部署路径标记为已验证。
