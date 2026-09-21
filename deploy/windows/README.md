# AICore Windows Deployment (Draft)

**状态**：Draft / 未在 Windows 环境验证。

## 计划

- Windows Service 包装 `aicored.exe`（通过 `windows-service` crate 或
  `sc.exe create` + 服务宿主 wrapper）。
- MSI 或 WiX 安装包（未实现；V3-M.2 或 V4）。
- 安装 / 升级 / 回滚脚本：PowerShell。

## 当前可用的最小脚本

`install.ps1` / `uninstall.ps1` 提供"以普通用户运行"的开发模式：
直接在后台启动 `aicored.exe`，日志写文件；不注册为 Windows Service。

## 服务注册（未来）

```powershell
sc.exe create aicored `
    binPath= "C:\Program Files\AICore\aicored.exe --interval-ms 100" `
    start= auto `
    DisplayName= "AICore Daemon"
sc.exe start aicored
已知限制
未注册为 Windows Service。

无 ACL 隔离。

无自动重启。
