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

Nightly 必须覆盖长稳；平台能力与真实 hardware tests 按 capability availability 分层执行。
