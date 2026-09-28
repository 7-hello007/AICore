[English](README.md) | [Chinese](README.zh-CN.md)

# AICore Deployment

## Directories

| Directory      | Contents                                                                     |
| -------------- | ---------------------------------------------------------------------------- |
| `linux/`     | Canonical systemd unit and install, uninstall, upgrade, and rollback scripts |
| `windows/`   | PowerShell deployment and Windows Service scripts                            |
| `packaging/` | Debian and tar archive packaging scripts                                     |

## Linux

```bash
cargo build --release --locked
mkdir -p deploy/linux/bin
cp target/release/aicored target/release/aicore target/release/aicore-dashboard deploy/linux/bin/
sudo ./deploy/linux/install.sh
```

`install.sh` renders the systemd unit with the selected `PREFIX`. `RuntimeDirectory=aicore` manages the lifecycle of `/run/aicore`. Resource limits depend on the deployment and should be supplied through a systemd drop-in instead of being hard-coded in the canonical unit.

## Packaging

```bash
./deploy/packaging/build_deb.sh
./deploy/packaging/build_tar.sh
```

## Windows

See `windows/README.md`. The repository contains development launch scripts and explicit Windows Service installation and removal scripts. Verification on the target host is required before release promotion.
