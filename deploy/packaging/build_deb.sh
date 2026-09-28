#!/usr/bin/env bash

# Builds the.deb package.
#
# Usage:
#   ./build_deb.sh # version read from workspace.package.version
#   AICORE_VERSION=0.3.2 ./build_deb.sh
#
# Prefix:
#   - Cargo built-release executed.
#   - dpkg-deb is available (Ubuntu / Debian belt).
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
mkdir -p "${STAGE}/usr/local/bin/models"
mkdir -p "${STAGE}/usr/local/share/doc/aicore"
mkdir -p "${STAGE}/etc/systemd/system"
mkdir -p "${STAGE}/etc/aicore"
mkdir -p "${STAGE}/usr/local/share/aicore/plugins"

# Binary
install -m 0755 "${TARGET_DIR}/aicored" "${STAGE}/usr/local/bin/aicored"
install -m 0755 "${TARGET_DIR}/aicore" "${STAGE}/usr/local/bin/aicore"
install -m 0644 "${REPO_ROOT}/models/policy_classifier_v1.json" \
    "${STAGE}/usr/local/bin/models/policy_classifier_v1.json"
install -m 0644 "${REPO_ROOT}/LICENSE" "${STAGE}/usr/local/share/doc/aicore/LICENSE"
install -m 0644 "${REPO_ROOT}/NOTICE" "${STAGE}/usr/local/share/doc/aicore/NOTICE"
install -m 0644 "${REPO_ROOT}/README.md" "${STAGE}/usr/local/share/doc/aicore/README.md"
[ -x "${TARGET_DIR}/aicore-dashboard" ] && \
    install -m 0755 "${TARGET_DIR}/aicore-dashboard" "${STAGE}/usr/local/bin/aicore-dashboard" || true

# systemd unit
install -m 0644 "${REPO_ROOT}/deploy/linux/aicored.service" \
    "${STAGE}/etc/systemd/system/aicored.service"

# Control script
install -m 0755 "${REPO_ROOT}/deploy/packaging/debian/postinst" "${STAGE}/DEBIAN/postinst"
install -m 0755 "${REPO_ROOT}/deploy/packaging/debian/prerm" "${STAGE}/DEBIAN/prerm"
install -m 0755 "${REPO_ROOT}/deploy/packaging/debian/postrm" "${STAGE}/DEBIAN/postrm"

# control file (template replacement)
sed -e "s/@VERSION@/${VERSION}/" -e "s/@ARCH@/${ARCH}/" \
    "${REPO_ROOT}/deploy/packaging/debian/control.template" \
    > "${STAGE}/DEBIAN/control"

# Pack up.
cd "${REPO_ROOT}/dist"
dpkg-deb --root-owner-group --build "${PKG_NAME}" "${PKG_NAME}.deb"

echo
echo "package: ${REPO_ROOT}/dist/${PKG_NAME}.deb"
ls -lh "${PKG_NAME}.deb"
