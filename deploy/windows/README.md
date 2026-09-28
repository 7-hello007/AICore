[English](README.md) | [Chinese](README.zh-CN.md)

# AICore Windows Deployment

**Status:** Implementation candidate; verification in Windows CI or on a target host is required.

## Operating Modes

- Development mode: run `aicored.exe` directly with the regular start and stop scripts.
- Windows Service: register and manage the service with `service-install.ps1` and the corresponding removal script.

The Windows Service path is implemented. Production deployment should still use a dedicated low-privilege service account and verify access control, automatic restart, upgrade and rollback behavior, and Named Pipe permissions.

## Release Gate

This deployment path can be marked as verified only after service installation, start, query, stop, removal, IPC, Dashboard, and upgrade and rollback smoke tests pass on a native Windows host.
