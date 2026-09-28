[English](TRACEABILITY.md) | [Chinese](TRACEABILITY.zh-CN.md)
# Requirement Traceability

```mermaid
flowchart LR
    REQ[Requirement]
    MOD[Module]
    API[Contract/API]
    CODE[Implementation]
    TEST[Test]
    ACCEPT[Acceptance]

    REQ --> MOD --> API --> CODE --> TEST --> ACCEPT
```

Every functional or nonfunctional requirement must trace to at least one implementation entry point and one item of test or acceptance evidence.
