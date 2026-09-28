[英文](CI_CD.md) | [中文](CI_CD.zh-CN.md)

# 持续集成与持续交付

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

夜间构建应覆盖长时间稳定性测试；平台能力与真实硬件测试按实际可用能力分层执行。
