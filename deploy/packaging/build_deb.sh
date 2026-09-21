#!/usr/bin/env bash

# 构建 .deb 包。
#
# 用法：
#   ./build_deb.sh             # 版本从 workspace.package.version 读取
#   AICORE_VERSION=0.3.2 ./build_deb.sh
#
# 前置：
#   - cargo build --release 已执行。
#   - dpkg-deb 可用（Ubuntu / Debian 自带）。
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
source "${REPO_ROOT}/scripts/lib/version.sh"

command -v cargo >/dev/null 2>&1 || { echo "error: cargo not found" >&2; exit 1; }

VERSION="${AICORE_VERSION:-${WORKSPACE_VERSION}}"
ARCH="$(dpkg --print-architecture 2>/dev/null || echo amd64)"
PKG_NAME="aicore_${VERSION}_${ARCH}"

STAGE="${REPO_ROOT}/dist/${PKG_NAME}"
TARGET_DIR="${REPO_ROOT}/target/release"

if [ ! -x "${TARGET_DIR}/aicored" ]; then
    echo "error: missing ${TARGET_DIR}/aicored; run 'cargo build --release' first" >&2
    exit 1
fi

if ! command -v dpkg-deb >/dev/null 2>&1; then
    echo "error: dpkg-deb not found" >&2
    exit 1
fi

rm -rf "${STAGE}"
mkdir -p "${STAGE}/DEBIAN"
mkdir -p "${STAGE}/usr/local/bin"
mkdir -p "${STAGE}/etc/systemd/system"
mkdir -p "${STAGE}/etc/aicore"
mkdir -p "${STAGE}/usr/local/share/aicore/plugins"

# 二进制
install -m 0755 "${TARGET_DIR}/aicored" "${STAGE}/usr/local/bin/aicored"
install -m 0755 "${TARGET_DIR}/aicore" "${STAGE}/usr/local/bin/aicore"
[ -x "${TARGET_DIR}/aicore-dashboard" ] && \
    install -m 0755 "${TARGET_DIR}/aicore-dashboard" "${STAGE}/usr/local/bin/aicore-dashboard" || true

# systemd unit
install -m 0644 "${REPO_ROOT}/deploy/linux/aicored.service" \
    "${STAGE}/etc/systemd/system/aicored.service"

# 控制脚本
install -m 0755 "${REPO_ROOT}/deploy/packaging/debian/postinst" "${STAGE}/DEBIAN/postinst"
install -m 0755 "${REPO_ROOT}/deploy/packaging/debian/prerm" "${STAGE}/DEBIAN/prerm"
install -m 0755 "${REPO_ROOT}/deploy/packaging/debian/postrm" "${STAGE}/DEBIAN/postrm"

# control 文件（模板替换）
sed -e "s/@VERSION@/${VERSION}/" -e "s/@ARCH@/${ARCH}/" \
    "${REPO_ROOT}/deploy/packaging/debian/control.template" \
    > "${STAGE}/DEBIAN/control"

# 打包
cd "${REPO_ROOT}/dist"
dpkg-deb --root-owner-group --build "${PKG_NAME}" "${PKG_NAME}.deb"

echo
echo "package: ${REPO_ROOT}/dist/${PKG_NAME}.deb"
ls -lh "${PKG_NAME}.deb"
