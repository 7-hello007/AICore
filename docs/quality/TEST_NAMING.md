[English](TEST_NAMING.md) | [Chinese](TEST_NAMING.zh-CN.md)
# Test Naming

The test directory must be flat: only `tests/`, and no subdirectories should be created by category.

Format:

`featureName.behavior[.detail].test.extension`

Rules:

- Use lower camel case for each name segment.
- Keep each description within five words where practical.
- Use a real module or capability name as the feature segment.
- Do not use placeholders such as `test1`, `tempTest`, or `newTest`.

Example:

```text
context.buildState.test.rs
policy.generatePolicy.test.rs
adapter.windowsProcess.test.rs
plugin.verifySignature.test.rs
```
