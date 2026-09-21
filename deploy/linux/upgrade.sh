#!/usr/bin/env bash
# AICore daemon 升级脚本（Linux）。
#
# 行为：
#   1. 备份当前二进制到 /var/backups/aicore/<timestamp>/。
#   2. 安装新二进制。
#   3. 重启服务。
#   4. 若健康检查失败，自动回滚到备份。
#
# 用法：
#   sudo ./upgrade.sh
set -euo pipefail

PREFIX="${PREFIX:-/usr/local}"
BIN_DIR="${PREFIX}/bin"
BACKUP_ROOT="/var/backups/aicore"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SOURCE_BIN_DIR="${SOURCE_BIN_DIR:-${SCRIPT_DIR}/bin}"

if [ "$(id -u)" -ne 0 ]; then
    echo "error: must run as root" >&2
    exit 1
fi

TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
BACKUP_DIR="${BACKUP_ROOT}/${TIMESTAMP}"
mkdir -p "${BACKUP_DIR}"

# 1. 备份当前二进制
for bin in aicored aicore aicore-dashboard; do
    if [ -f "${BIN_DIR}/${bin}" ]; then
        cp -a "${BIN_DIR}/${bin}" "${BACKUP_DIR}/${bin}"
        echo "backed up ${bin} → ${BACKUP_DIR}/${bin}"
    fi
done

# 2. 安装新二进制
for bin in aicored aicore aicore-dashboard; do
    if [ ! -f "${SOURCE_BIN_DIR}/${bin}" ]; then
        echo "error: missing ${SOURCE_BIN_DIR}/${bin}" >&2
        echo "rolling back (nothing changed yet)"
        exit 1
    fi
    install -m 0755 "${SOURCE_BIN_DIR}/${bin}" "${BIN_DIR}/${bin}"
done

# 3. 重启服务
systemctl daemon-reload
systemctl restart aicored.service

# 4. 健康检查（10s 内必须 active）
HEALTHY=false
for i in $(seq 1 10); do
    sleep 1
    if systemctl is-active --quiet aicored.service; then
        HEALTHY=true
        break
    fi
done

if [ "$HEALTHY" = false ]; then
    echo "health check failed; rolling back" >&2
    for bin in aicored aicore aicore-dashboard; do
        if [ -f "${BACKUP_DIR}/${bin}" ]; then
            cp -a "${BACKUP_DIR}/${bin}" "${BIN_DIR}/${bin}"
        fi
    done
    systemctl restart aicored.service
    echo "rolled back to ${BACKUP_DIR}" >&2
    exit 1
fi

echo
echo "upgrade complete."
echo "backup kept at ${BACKUP_DIR}"
echo "to rollback manually: sudo ${SCRIPT_DIR}/rollback.sh ${TIMESTAMP}"
