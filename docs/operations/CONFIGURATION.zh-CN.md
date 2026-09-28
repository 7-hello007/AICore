[英文](CONFIGURATION.md) | [中文](CONFIGURATION.zh-CN.md)

# 配置

配置必须可：

- 文件加载
- 环境变量覆盖
- 命令行临时覆盖
- 版本迁移
- 校验
- 安全回退默认值

配置优先级：`CLI > Environment > Config File > Defaults`。
