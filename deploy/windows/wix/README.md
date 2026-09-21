# WiX MSI Template

## 前置

- Windows 环境
- WiX Toolset v3（`candle` + `light`）

## 构建

```powershell
# 1. 构建 release 二进制
cargo build --release -p aicore-daemon -p aicore-cli

# 2. 生成 MSI
cd deploy/windows/wix
candle -dVersion=<workspace-version> -dBinDir=..\..\..\target\release aicore.wxs
light -out ..\..\..\dist\aicore-<workspace-version>.msi aicore.wixobj
输出
dist/aicore-<version>.msi

已知限制
Web Dashboard 是独立进程；发布包应与 daemon 一起分发。

未配置代码签名；生产环境应配置 Authenticode。
