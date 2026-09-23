# AICore Deployment

## 目录

| 目录 | 内容 |
|---|---|
| `linux/` | canonical systemd unit + install/uninstall/upgrade/rollback scripts |
| `windows/` | PowerShell deployment and Windows Service scripts |
| `packaging/` | Debian and tar packaging scripts |

## Linux

```bash
cargo build --release --locked
mkdir -p deploy/linux/bin
cp target/release/aicored target/release/aicore target/release/aicore-dashboard deploy/linux/bin/
sudo ./deploy/linux/install.sh
```

`install.sh` renders the systemd unit using the selected `PREFIX`; `RuntimeDirectory=aicore` owns `/run/aicore` lifecycle. Resource limits are deployment-specific and should be supplied with a systemd drop-in rather than hard-coded into the canonical unit.

## Packaging

```bash
./deploy/packaging/build_deb.sh
./deploy/packaging/build_tar.sh
```

## Windows

See `windows/README.md`. The repository contains both development launch scripts and explicit Windows Service install/uninstall scripts. Target-host verification is required before release promotion.
