# IPC Protocol

```mermaid
sequenceDiagram
    participant C as Client
    participant S as AICore Service
    C->>S: Handshake(version, capabilities)
    S-->>C: HandshakeAck
    C->>S: Request(requestId, action, payload)
    S-->>C: Ack(requestId)
    S-->>C: Response(requestId, result)
```

## 必须具备

- `protocol_version`
- `request_id`
- `action`
- capability handshake
- Ack
- duplicate request protection
- bounded concurrency/backpressure
- local authentication/permission
- timeout
- deterministic error model
