#!/usr/bin/env bash
# AICore daemon 回滚脚本（Linux）。
#
# 用法：
#   sudo ./rollback.sh                 # 列出可用备份
#   sudo ./rollback.sh <timestamp>     # 回滚到指定备份
set -euo pipefail

PREFIX="${PREFIX:-/usr/local}"
BIN_DIR="${PREFIX}/bin"
BACKUP_ROOT="/var/backups/aicore"

if [ "$(id -u)" -ne 0 ]; then
    echo "error: must run as root" >&2
    exit 1
fi

if [ $# -eq 0 ]; then
    echo "available backups:"
    ls -1 "${BACKUP_ROOT}" 2>/dev/null || echo "  (none)"
    echo
    echo "usage: sudo $0 <timestamp>"
    exit 0
fi

TIMESTAMP="$1"
BACKUP_DIR="${BACKUP_ROOT}/${TIMESTAMP}"

if [ ! -d "${BACKUP_DIR}" ]; then
    echo "error: backup not found: ${BACKUP_DIR}" >&2
    exit 1
fi

echo "rolling back to ${BACKUP_DIR}"
for bin in aicored aicore aicore-dashboard; do
    if [ -f "${BACKUP_DIR}/${bin}" ]; then
        cp -a "${BACKUP_DIR}/${bin}" "${BIN_DIR}/${bin}"
        echo "restored ${bin}"
    fi
done
if [ -f "${BACKUP_DIR}/models/policy_classifier_v1.json" ]; then
    install -d -m 0755 "${BIN_DIR}/models"
    cp -a "${BACKUP_DIR}/models/policy_classifier_v1.json" \
        "${BIN_DIR}/models/policy_classifier_v1.json"
else
    rm -f "${BIN_DIR}/models/policy_classifier_v1.json"
fi

systemctl restart aicored.service
sleep 2
if systemctl is-active --quiet aicored.service; then
    echo "rollback complete; service active"
else
    echo "warning: service is not active after rollback" >&2
    exit 1
fi
