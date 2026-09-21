#!/usr/bin/env bash
# AICore daemon 安装脚本（Linux）。
#
# 用法：
#   sudo ./install.sh                     # 从当前目录的 bin/ 安装
#   sudo PREFIX=/opt/aicore ./install.sh  # 自定义前缀（默认 /usr/local）
#
# 行为：
#   - 创建 aicore 系统用户与组。
#   - 安装二进制到 $PREFIX/bin。
#   - 安装 systemd unit。
#   - 创建状态目录 /var/lib/aicore、运行时目录 /run/aicore。
#   - 若配置不存在，写入默认配置文件。
#   - 启用并启动 aicored.service。
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

# 1. 检查 root
if [ "$(id -u)" -ne 0 ]; then
    echo "error: must run as root (use sudo)" >&2
    exit 1
fi

# 2. 检查二进制
for bin in aicored aicore aicore-dashboard; do
    if [ ! -f "${SOURCE_BIN_DIR}/${bin}" ]; then
        echo "error: missing binary ${SOURCE_BIN_DIR}/${bin}" >&2
        echo "hint: run 'cargo build --release' and copy binaries to ${SOURCE_BIN_DIR}" >&2
        exit 1
    fi
done

# 3. 创建用户与组（若不存在）
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

# 4. 目录
install -d -m 0755 "${BIN_DIR}"
install -d -m 0755 "${UNIT_DIR}"
install -d -m 0750 -o aicore -g aicore "${STATE_DIR}"
install -d -m 0750 -o aicore -g aicore "${RUNTIME_DIR}"
install -d -m 0755 "${CONFIG_DIR}"
install -d -m 0755 "${SHARE_DIR}"
install -d -m 0755 "${SHARE_DIR}/plugins"

# 5. 二进制
for bin in aicored aicore aicore-dashboard; do
    install -m 0755 "${SOURCE_BIN_DIR}/${bin}" "${BIN_DIR}/${bin}"
    echo "installed ${BIN_DIR}/${bin}"
done

# 6. systemd unit
install -m 0644 "${SCRIPT_DIR}/aicored.service" "${UNIT_DIR}/aicored.service"
echo "installed ${UNIT_DIR}/aicored.service"

# 7. 默认配置（若不存在）
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

# 8. 重载并启动
systemctl daemon-reload
systemctl enable aicored.service
systemctl restart aicored.service

echo
echo "install complete."
echo "check:    systemctl status aicored"
echo "logs:     journalctl -u aicored -f"
echo "cli:      aicore status"
