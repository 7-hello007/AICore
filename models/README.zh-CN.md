[英文](README.md) | [中文](README.zh-CN.md)

# AICore 本地模型

V3.2 生产基线使用以下确定性本地特征加权模型：

- `policy_classifier_v1.json`

`PureRustRuntime` 无需外部模型服务器即可加载该模型。

ONNX 仍是可选扩展点。仓库没有把占位 `.onnx` 文件描述为可用模型。
