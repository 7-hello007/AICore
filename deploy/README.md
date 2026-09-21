# AICore Deployment

## 目录

| 目录 | 内容 |
|---|---|
| `linux/` | systemd unit + install/uninstall/upgrade/rollback 脚本 |
| `windows/` | 开发模式 PowerShell 脚本（Draft，未验证） |
| `packaging/` | deb 与 tar 打包脚本 |
| `linux/` | canonical systemd unit + installer/upgrade/rollback |

## Linux 快速安装

```bash
cargo build --release
mkdir -p deploy/linux/bin
cp target/release/aicored target/release/aicore target/release/aicore-dashboard deploy/linux/bin/
sudo ./deploy/linux/install.sh
Linux 打包
bash
cargo build --release
./deploy/packaging/build_deb.sh
./deploy/packaging/build_tar.sh
Linux 升级 / 回滚
bash
sudo ./deploy/linux/upgrade.sh
sudo ./deploy/linux/rollback.sh                # 列出备份
sudo ./deploy/linux/rollback.sh 20250920T120000Z
Windows
见 windows/README.md。目前仅开发模式，未注册为 Windows Service。
