[English](README.md) | [Chinese](README.zh-CN.md)

# AICore Local Models

The V3.2 production baseline uses this deterministic local feature-weighted model:

- `policy_classifier_v1.json`

`PureRustRuntime` loads the model without an external model server.

ONNX remains an optional extension point. The repository does not include a placeholder `.onnx` file presented as a working model.
