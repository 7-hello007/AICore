# Build & Release

版本唯一来源：`workspace.package.version`。

所有包装脚本通过统一版本读取逻辑生成：DEB/TAR/MSI 等。

发布前门禁：

1. format
2. clippy
3. unit/contract/integration tests
4. platform build
5. package smoke test
6. security checks
7. documentation consistency
