[英文](BUILD_RELEASE.md) | [中文](BUILD_RELEASE.zh-CN.md)

# 构建与发布

版本唯一来源：`workspace.package.version`。

DEB、TAR、MSI 等打包脚本统一读取版本号。

发布前门禁：

1. 格式检查
2. 静态检查
3. 单元、契约与集成测试
4. 平台构建
5. 安装包冒烟测试
6. 安全检查
7. 文档一致性检查
