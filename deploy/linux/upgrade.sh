#!/usr/bin/env bash
# AICore daemon Upgrade Script (Linux).
#
# Conduct:
#   1. Backup current binary to /var/backups/aicore/<timestamp>/.
#   2. Installation of a new binary.
#   3. Restart services.
#   If a health check fails, roll back automatically to the backup.
#
# Usage:
#   sudo ./upgrade.sh
set -euo pipefail

PREFIX="${PREFIX:-/usr/local}"
BIN_DIR="${PREFIX}/bin"
BACKUP_ROOT="/var/backups/aicore"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SOURCE_BIN_DIR="${SOURCE_BIN_DIR:-${SCRIPT_DIR}/bin}"
MODEL_REL="models/policy_classifier_v1.json"

if [ "$(id -u)" -ne 0 ]; then
    echo "error: must run as root" >&2
    exit 1
fi
for source in "${SOURCE_BIN_DIR}/aicored" "${SOURCE_BIN_DIR}/aicore" \
    "${SOURCE_BIN_DIR}/aicore-dashboard" "${SOURCE_BIN_DIR}/${MODEL_REL}"; do
    if [ ! -f "${source}" ]; then
        echo "error: missing ${source}" >&2
        exit 1
    fi
done

TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
BACKUP_DIR="${BACKUP_ROOT}/${TIMESTAMP}"
mkdir -p "${BACKUP_DIR}"

# 1. Backup Current Binary
for bin in aicored aicore aicore-dashboard; do
    if [ -f "${BIN_DIR}/${bin}" ]; then
        cp -a "${BIN_DIR}/${bin}" "${BACKUP_DIR}/${bin}"
        echo "backed up ${bin} → ${BACKUP_DIR}/${bin}"
    fi
done
if [ -f "${BIN_DIR}/${MODEL_REL}" ]; then
    mkdir -p "${BACKUP_DIR}/models"
    cp -a "${BIN_DIR}/${MODEL_REL}" "${BACKUP_DIR}/${MODEL_REL}"
fi

# 2. Installation of a new binary
for bin in aicored aicore aicore-dashboard; do
    if [ ! -f "${SOURCE_BIN_DIR}/${bin}" ]; then
        echo "error: missing ${SOURCE_BIN_DIR}/${bin}" >&2
        echo "rolling back (nothing changed yet)"
        exit 1
    fi
    install -m 0755 "${SOURCE_BIN_DIR}/${bin}" "${BIN_DIR}/${bin}"
done
install -d -m 0755 "${BIN_DIR}/models"
install -m 0644 "${SOURCE_BIN_DIR}/${MODEL_REL}" "${BIN_DIR}/${MODEL_REL}"

# 3. Restarting services
systemctl daemon-reload
systemctl restart aicored.service

# Health check-ups (10s)
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
    if [ -f "${BACKUP_DIR}/${MODEL_REL}" ]; then
        cp -a "${BACKUP_DIR}/${MODEL_REL}" "${BIN_DIR}/${MODEL_REL}"
    else
        rm -f "${BIN_DIR}/${MODEL_REL}"
    fi
    systemctl restart aicored.service
    echo "rolled back to ${BACKUP_DIR}" >&2
    exit 1
fi

echo
echo "upgrade complete."
echo "backup kept at ${BACKUP_DIR}"
echo "to rollback manually: sudo ${SCRIPT_DIR}/rollback.sh ${TIMESTAMP}"
