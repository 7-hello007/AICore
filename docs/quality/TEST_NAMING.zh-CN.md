[英文](TEST_NAMING.md) | [中文](TEST_NAMING.zh-CN.md)

# 测试命名

测试目录必须扁平：只有 `tests/`，不得按类别创建子目录。

格式：

`功能名.功能说明[.功能补充说明].test.后缀名`

规则：

- 驼峰式命名
- 说明尽量不超过 5 个单词
- 功能名必须对应真实模块/能力
- 禁止 `test1` / `tempTest` / `newTest`

示例：

```text
context.buildState.test.rs
policy.generatePolicy.test.rs
adapter.windowsProcess.test.rs
plugin.verifySignature.test.rs
```
