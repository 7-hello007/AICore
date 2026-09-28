[English](BUILD_RELEASE.md) | [Chinese](BUILD_RELEASE.zh-CN.md)
# Build & Release

The single version source is `workspace.package.version`.

Packaging scripts for DEB, TAR, and MSI read that version through one shared mechanism.

Release gates:

1. format
2. clippy
3. unit/contract/integration tests
4. platform build
5. package smoke test
6. security checks
7. documentation consistency
