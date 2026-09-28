#!/usr/bin/env bash

# Builds .tar.gz packages (for non-deb systems).
#
# Usage:
#   ./build_tar.sh # version read from workspace
#   AICORE_VERSION = <version>./build_tar.sh # Visible overlay version
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
source "${REPO_ROOT}/scripts/lib/version.sh"

command -v cargo >/dev/null 2>&1 || { echo "error: cargo not found" >&2; exit 1; }

VERSION="${AICORE_VERSION:-${WORKSPACE_VERSION}}"
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"
PKG_NAME="aicore-${VERSION}-${OS}-${ARCH}"

STAGE="${REPO_ROOT}/dist/${PKG_NAME}"
TARGET_DIR="${REPO_ROOT}/target/release"

rm -rf "${STAGE}"
mkdir -p "${STAGE}/bin"
mkdir -p "${STAGE}/bin/models"

install -m 0755 "${TARGET_DIR}/aicored" "${STAGE}/bin/"
install -m 0755 "${TARGET_DIR}/aicore" "${STAGE}/bin/"
install -m 0644 "${REPO_ROOT}/models/policy_classifier_v1.json" \
    "${STAGE}/bin/models/policy_classifier_v1.json"
install -m 0644 "${REPO_ROOT}/LICENSE" "${STAGE}/LICENSE"
install -m 0644 "${REPO_ROOT}/NOTICE" "${STAGE}/NOTICE"
install -m 0644 "${REPO_ROOT}/README.md" "${STAGE}/README.md"
[ -x "${TARGET_DIR}/aicore-dashboard" ] && \
    install -m 0755 "${TARGET_DIR}/aicore-dashboard" "${STAGE}/bin/" || true

cp "${REPO_ROOT}/deploy/linux/install.sh"   "${STAGE}/"
cp "${REPO_ROOT}/deploy/linux/uninstall.sh" "${STAGE}/"
cp "${REPO_ROOT}/deploy/linux/upgrade.sh"   "${STAGE}/"
cp "${REPO_ROOT}/deploy/linux/rollback.sh"  "${STAGE}/"
cp "${REPO_ROOT}/deploy/linux/aicored.service" "${STAGE}/"

cd "${REPO_ROOT}/dist"
tar -czf "${PKG_NAME}.tar.gz" "${PKG_NAME}"
echo
echo "package: ${REPO_ROOT}/dist/${PKG_NAME}.tar.gz"
ls -lh "${PKG_NAME}.tar.gz"
