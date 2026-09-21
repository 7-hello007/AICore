#!/usr/bin/env bash
# 从 workspace 的 Cargo.toml 读取规范版本。
#
# 用法：
#   REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
#   source "${REPO_ROOT}/scripts/lib/version.sh"
#   echo "$WORKSPACE_VERSION"
#
# 设计意图：
#   - workspace.package.version 是版本号的唯一来源。
#   - 所有构建脚本必须通过此库读取，禁止硬编码版本号。
#   - 用户可通过 AICORE_VERSION 环境变量显式覆盖。

# 解析库脚本自身的位置，以便在不依赖当前工作目录的情况下找到仓库根目录。
_version_lib_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
_version_repo_root="$(cd "${_version_lib_dir}/../.." && pwd)"

# 从给定 manifest 中提取 [workspace.package] 下的 version 字段。
# 参数：
#   $1 - 可选的 Cargo.toml 路径；默认使用仓库根目录的 Cargo.toml。
# 返回值：
#   成功时输出版本字符串（例如 "0.1.0"）；失败时输出空字符串。
get_workspace_version() {
    local manifest="${1:-${_version_repo_root}/Cargo.toml}"
    if [ ! -f "$manifest" ]; then
        echo "错误：找不到 manifest: $manifest" >&2
        return 1
    fi
    sed -n '/^\[workspace\.package\]/,/^\[/p' "$manifest" \
        | grep -E '^version[[:space:]]*=' \
        | head -n 1 \
        | sed -E 's/.*"([^"]*)".*/\1/'
}

# 加载时立即解析版本，并导出到环境。
WORKSPACE_VERSION="$(get_workspace_version)"
if [ -z "$WORKSPACE_VERSION" ]; then
    echo "错误：无法从 ${_version_repo_root}/Cargo.toml 读取 workspace.package.version" >&2
    return 1 2>/dev/null || exit 1
fi

export WORKSPACE_VERSION