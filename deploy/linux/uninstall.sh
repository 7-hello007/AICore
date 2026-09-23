#!/usr/bin/env bash
# AICore daemon 卸载脚本（Linux）。
#
# 用法：
#   sudo ./uninstall.sh           # 保留数据
#   sudo ./uninstall.sh --purge   # 删除用户与数据
set -euo pipefail

PREFIX="${PREFIX:-/usr/local}"
BIN_DIR="${PREFIX}/bin"
UNIT_DIR="/etc/systemd/system"
STATE_DIR="/var/lib/aicore"
RUNTIME_DIR="/run/aicore"
CONFIG_DIR="/etc/aicore"
SHARE_DIR="${PREFIX}/share/aicore"

PURGE=false
for arg in "$@"; do
    case "$arg" in
        --purge) PURGE=true ;;
        -h|--help)
            echo "usage: sudo $0 [--purge]"
            exit 0
            ;;
        *) echo "unknown arg: $arg" >&2; exit 2 ;;
    esac
done

if [ "$(id -u)" -ne 0 ]; then
    echo "error: must run as root" >&2
    exit 1
fi

# 1. 停服务
systemctl stop aicored.service 2>/dev/null || true
systemctl disable aicored.service 2>/dev/null || true

# 2. 移除 unit
rm -f "${UNIT_DIR}/aicored.service"
systemctl daemon-reload 2>/dev/null || true

# 3. 移除二进制
for bin in aicored aicore aicore-dashboard; do
    rm -f "${BIN_DIR}/${bin}"
done
rm -f "${BIN_DIR}/models/policy_classifier_v1.json"
rmdir "${BIN_DIR}/models" 2>/dev/null || true

# 4. 移除 plugins 与 share
rm -rf "${SHARE_DIR}"

if [ "$PURGE" = true ]; then
    rm -rf "${STATE_DIR}"
    rm -rf "${RUNTIME_DIR}"
    rm -rf "${CONFIG_DIR}"

    if getent passwd aicore >/dev/null; then
        userdel aicore 2>/dev/null || true
    fi
    if getent group aicore >/dev/null; then
        groupdel aicore 2>/dev/null || true
    fi
    echo "purged data, config, user, group"
else
    echo "kept state (${STATE_DIR}), config (${CONFIG_DIR}), user (aicore)"
    echo "run with --purge to remove them"
fi

echo "uninstall complete."
