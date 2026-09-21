# Threat Model

主要威胁：

| Threat | Control |
|---|---|
| Malicious Plugin | Signature + Trust + Isolation |
| Prompt/Task Injection | Validator + Capability boundaries |
| Malformed IPC | Schema validation + request ID + limits |
| Rogue Local Process | Permission / identity checks |
| Model Corruption | Digest + signature + rollback |
| AI Wrong Decision | Deterministic validator + safe mode |
