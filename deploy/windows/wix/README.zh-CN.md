[英文](README.md) | [中文](README.zh-CN.md)

# WiX MSI 模板

## 前置条件

- Windows 环境
- WiX Toolset v3（`candle` 和 `light`）

## 构建

```powershell
# 1. 构建发布版二进制文件
cargo build --release -p aicore-daemon -p aicore-cli

# 2. 生成 MSI 安装包
cd deploy/windows/wix
candle -dVersion=<workspace-version> -dBinDir=..\..\..\target\release aicore.wxs
light -out ..\..\..\dist\aicore-<workspace-version>.msi aicore.wixobj
```

## 输出

`dist/aicore-<version>.msi`

## 已知限制

- Web Dashboard 是独立进程，应与守护进程一起分发。
- 尚未配置代码签名；生产安装包应使用 Authenticode。
