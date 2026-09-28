[English](README.md) | [Chinese](README.zh-CN.md)

# WiX MSI Template

## Prerequisites

- Windows environment
- WiX Toolset v3 (`candle` and `light`)

## Build

```powershell
# 1. Build the release binaries
cargo build --release -p aicore-daemon -p aicore-cli

# 2. Generate the MSI package
cd deploy/windows/wix
candle -dVersion=<workspace-version> -dBinDir=..\..\..\target\release aicore.wxs
light -out ..\..\..\dist\aicore-<workspace-version>.msi aicore.wixobj
```

## Output

`dist/aicore-<version>.msi`

## Known Limitations

- The Web Dashboard is a separate process and should be distributed with the daemon.
- Code signing is not configured; production packages should use Authenticode.
