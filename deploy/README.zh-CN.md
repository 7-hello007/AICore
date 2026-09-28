[英文](README.md) | [中文](README.zh-CN.md)

# AICore 部署

## 目录

| 目录 | 内容 |
|---|---|
| `linux/` | 标准 systemd 单元，以及安装、卸载、升级和回滚脚本 |
| `windows/` | PowerShell 部署脚本和 Windows Service 脚本 |
| `packaging/` | Debian 包和压缩归档打包脚本 |

## Linux

```bash
cargo build --release --locked
mkdir -p deploy/linux/bin
cp target/release/aicored target/release/aicore target/release/aicore-dashboard deploy/linux/bin/
sudo ./deploy/linux/install.sh
```

`install.sh` 使用选定的 `PREFIX` 生成 systemd 单元。`RuntimeDirectory=aicore` 管理 `/run/aicore` 的生命周期。资源限制取决于部署环境，应通过 systemd 附加配置提供，不应硬编码到标准单元中。

## 打包

```bash
./deploy/packaging/build_deb.sh
./deploy/packaging/build_tar.sh
```

## Windows

参见 `windows/README.zh-CN.md`。仓库包含开发启动脚本，以及明确的 Windows Service 安装和卸载脚本。发布晋级前必须在目标主机上完成验证。
