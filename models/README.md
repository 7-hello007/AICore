# AICore Local Models

The V3.2 production baseline uses the deterministic local feature-weighted model:

- `policy_classifier_v1.json`

It is loaded by `PureRustRuntime` and therefore requires no external model server.

ONNX remains an optional extension point; the repository does not ship a fake `.onnx` placeholder.
