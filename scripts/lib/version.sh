#!/usr/bin/env bash
# Read the canonical workspace version from Cargo.toml.
#
# Usage:
#   REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
#   source "${REPO_ROOT}/scripts/lib/version.sh"
#   echo "$WORKSPACE_VERSION"
#
# Design intent:
#   - workspace.package.version is the only default version source.
#   - Build scripts must use this library instead of hard-coding a version.
#   - AICORE_VERSION provides an explicit override.

# Resolve the repository root from this script rather than the current directory.
_version_lib_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
_version_repo_root="$(cd "${_version_lib_dir}/../.." && pwd)"

# Extract the version field from `[workspace.package]` in a manifest.
# Parameters:
#   $1 - optional Cargo.toml path; defaults to the repository-root Cargo.toml.
# Return value:
#   Print the version string on success (for example, "0.1.0"); print nothing on failure.
get_workspace_version() {
    local manifest="${1:-${_version_repo_root}/Cargo.toml}"
    if [ ! -f "$manifest" ]; then
        echo "Error: manifest not found: $manifest" >&2
        return 1
    fi
    sed -n '/^\[workspace\.package\]/,/^\[/p' "$manifest" \
        | grep -E '^version[[:space:]]*=' \
        | head -n 1 \
        | sed -E 's/.*"([^"]*)".*/\1/'
}

# Resolve and export the version when this library is sourced.
WORKSPACE_VERSION="${AICORE_VERSION:-$(get_workspace_version)}"
if [ -z "$WORKSPACE_VERSION" ]; then
    echo "Error: cannot read from ${_version_repo_root}/Cargo.toml workspace.package.version" >&2
    return 1 2>/dev/null || exit 1
fi

export WORKSPACE_VERSION
