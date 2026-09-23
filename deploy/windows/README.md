# AICore Windows Deployment

**状态**：实现候选；必须在 Windows CI / 目标机验证。

## 两种运行方式

- 开发模式：使用普通启动/停止脚本直接运行 `aicored.exe`。
- Windows Service：使用 `service-install.ps1` / 对应卸载脚本注册和管理服务。

Windows Service 路径已经存在，因此文档不再声明“未注册为 Windows Service”。生产部署仍应使用专用低权限服务账户，并验证 ACL、自动重启、升级/回滚和 Named Pipe 权限。

## Release gate

在真实 Windows 主机执行 service install/start/query/stop/uninstall、IPC、Dashboard 和 upgrade/rollback smoke 后，才能把该部署路径标记为已验证。
