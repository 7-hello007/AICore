#!/usr/bin/env bash
# AICore daemon installs scripts (Linux).
#
# Usage:
#   sido./install.sh #bin/ Install from current directory
#   Sudo PREFIX=/opt/aicore./install.sh # Custom prefix (default /usr/local)
#
# Conduct:
#   - Creates aicore system users and groups.
#   - Install binary to $PREFIX/bin.
#   - Install systemd unit.
#   - Creates a state directory /var/lib/aicore, a runtime directory /run/aicore.
#   - Write the default profile if configuration does not exist.
#   - Enable and start acored.service.
set -euo pipefail

PREFIX="${PREFIX:-/usr/local}"
BIN_DIR="${PREFIX}/bin"
UNIT_DIR="/etc/systemd/system"
STATE_DIR="/var/lib/aicore"
RUNTIME_DIR="/run/aicore"
CONFIG_DIR="/etc/aicore"
SHARE_DIR="${PREFIX}/share/aicore"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SOURCE_BIN_DIR="${SOURCE_BIN_DIR:-${SCRIPT_DIR}/bin}"

# 1. Inspection root
if [ "$(id -u)" -ne 0 ]; then
    echo "error: must run as root (use sudo)" >&2
    exit 1
fi

# 2. Check binary
for bin in aicored aicore aicore-dashboard; do
    if [ ! -f "${SOURCE_BIN_DIR}/${bin}" ]; then
        echo "error: missing binary ${SOURCE_BIN_DIR}/${bin}" >&2
        echo "hint: run 'cargo build --release' and copy binaries to ${SOURCE_BIN_DIR}" >&2
        exit 1
    fi
done
if [ ! -f "${SOURCE_BIN_DIR}/models/policy_classifier_v1.json" ]; then
    echo "error: missing ${SOURCE_BIN_DIR}/models/policy_classifier_v1.json" >&2
    exit 1
fi

# Create users and groups (if they do not exist)
if ! getent group aicore >/dev/null; then
    groupadd --system aicore
    echo "created group aicore"
fi
if ! getent passwd aicore >/dev/null; then
    useradd --system \
        --gid aicore \
        --home-dir "${STATE_DIR}" \
        --shell /usr/sbin/nologin \
        --comment "AICore daemon" \
        aicore
    echo "created user aicore"
fi

# Contents
install -d -m 0755 "${BIN_DIR}"
install -d -m 0755 "${BIN_DIR}/models"
install -d -m 0755 "${UNIT_DIR}"
install -d -m 0750 -o aicore -g aicore "${STATE_DIR}"
install -d -m 0755 "${CONFIG_DIR}"
install -d -m 0755 "${SHARE_DIR}"
install -d -m 0755 "${SHARE_DIR}/plugins"

# Binary
for bin in aicored aicore aicore-dashboard; do
    install -m 0755 "${SOURCE_BIN_DIR}/${bin}" "${BIN_DIR}/${bin}"
    echo "installed ${BIN_DIR}/${bin}"
done
install -m 0644 "${SOURCE_BIN_DIR}/models/policy_classifier_v1.json" \
    "${BIN_DIR}/models/policy_classifier_v1.json"

# 6. systemd unit
sed \
    -e "s|/usr/local/bin/aicored|${BIN_DIR}/aicored|g" \
    -e "s|/usr/local/share/aicore|${SHARE_DIR}|g" \
    "${SCRIPT_DIR}/aicored.service" > "${UNIT_DIR}/aicored.service"
chmod 0644 "${UNIT_DIR}/aicored.service"
echo "installed ${UNIT_DIR}/aicored.service (PREFIX=${PREFIX})"

# Default configuration (if not available)
if [ ! -f "${CONFIG_DIR}/config.json" ]; then
    cat > "${CONFIG_DIR}/config.json" << 'JSON'
{
  "version": "1.0",
  "daemon": {
    "telemetry": { "sampling_interval_ms": 100 },
    "state_write_interval_ticks": 10,
    "shutdown_timeout_ms": 5000,
    "initial_mode": "normal"
  },
  "plugins_dir": null,
  "memory_path": null,
  "logging_level": "info",
  "logging_json": true
}
JSON
    chmod 0644 "${CONFIG_DIR}/config.json"
    echo "wrote default config ${CONFIG_DIR}/config.json"
fi

# 8. Reload and start
systemctl daemon-reload
systemctl enable aicored.service
systemctl restart aicored.service

echo
echo "install complete."
echo "check:    systemctl status aicored"
echo "logs:     journalctl -u aicored -f"
echo "cli:      aicore status"
