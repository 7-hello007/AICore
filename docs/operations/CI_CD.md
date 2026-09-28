[English](CI_CD.md) | [Chinese](CI_CD.zh-CN.md)
# CI/CD

```mermaid
flowchart LR
    PR[Pull Request]
    FORMAT[Format]
    CLIPPY[Clippy]
    TEST[Test Matrix]
    PLATFORM[Windows/Linux]
    SECURITY[Security]
    PACKAGE[Package]
    RELEASE[Release Gate]

    PR --> FORMAT --> CLIPPY --> TEST --> PLATFORM --> SECURITY --> PACKAGE --> RELEASE
```

Nightly runs should cover long-term stability. Run platform and real-hardware tests according to the capabilities available on each test machine.
