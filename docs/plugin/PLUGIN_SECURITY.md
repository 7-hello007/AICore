# Plugin Security

必须支持：

- artifact digest
- Ed25519 signature
- key id / trust store
- explicit permissions
- process isolation where required
- resource limits
- timeout enforcement
- failure counter
- disable / rollback

Local-development bypass 不得等同于 production trust。
