# Local intent providers

Core consumes `IIntentAgent` and validated `IntentProposal`; backend selection,
token vocabulary and model placement remain outside Core. Intent output is
semantic input to the existing policy engine/validator, never permission to
actuate hardware. `configured_intent_agent` is shared by daemon and smoke
example.

| `AICORE_INTENT_BACKEND` | Actual implementation | Dependencies |
|---|---|---|
| `ollama` (default) | Loopback Ollama HTTP inference | Already-running Ollama; `AICORE_OLLAMA_MODEL` defaults to `llama3.2:3b` |
| `json` | `PureRustRuntime` weighted-token classification | `AICORE_INTENT_MODEL_PATH`, default `models/intent_classifier_v1.json` |
| `onnx` | Explicit ONNX Runtime subprocess via `PythonOnnxSessionFactory` (CPU default) | Explicit `AICORE_ONNX_PYTHON`; packages in project `env/onnx-packages` |
| `ensemble` | Concurrent bounded JSON and Ollama proposals, weighted consensus | Both providers above; disagreement/tardy proposals fail |
| `disabled` | No intent provider | API returns NotSupported |

Set `AICORE_PROJECT_ROOT` to the absolute deployment project root when relocating
the binary. The build checkout root is the development fallback. ONNX model
path can be selected independently by `AICORE_ONNX_MODEL_PATH`; its default is
the JSON manifest path with `.onnx` extension. Vocabulary length, actual input
and output tensor shapes and F32 types must match the manifest. Labels are
typed workload names. Providers reject unknown input, invalid probabilities,
expired proposals and confidence below the current validation threshold.

The supplied JSON model is a **hand-authored classification baseline**, not a
trained LLM or calibrated accuracy claim. `scripts/models/export_intent_onnx.py`
exports the same weights as MatMul + Softmax; the checked-in ONNX file is that
export. Changing model/vocabulary does not require editing Core. Arbitrary
generative ONNX LLMs require a different tokenizer/generation adapter; this
classifier does not claim to support them.

The ONNX bridge invokes an existing Python interpreter with `-I -S`, inserts
only project-local packages, defaults to `CPUExecutionProvider`, validates actual
model schemas, drains stdout/stderr, bounds protocol bytes and kills timed-out
subprocesses. It never downloads dependencies or falls back to mock.
`AICORE_ONNX_PROVIDER` selects CPU, `DmlExecutionProvider`, or
`VitisAIExecutionProvider`; `AICORE_ONNX_DEVICE_ID` supplies an explicit device
ordinal. DirectML requires the DXGI ordinal. NPU selection enumerates actual EP
NPU devices and requires a valid ordinal, rather than guessing from a vendor.
`AICORE_ONNX_PACKAGES` selects the isolated package directory and optional
`AICORE_ONNX_PROVIDER_LIBRARY` names an already installed absolute EP DLL path.
Library registration is process-local and explicitly unregistered on normal
success/failure; a killed subprocess releases its own state. No Store/provider
automatic install or download API is called. Accelerator sessions disable CPU
fallback and require profiling evidence that nodes execute on the requested EP.
Scratch output and NPU model cache stay under the package directory's project
`env/` parent. Sessions are created per subprocess; this is not persistent serving.

For daemon policy-model device queues, use `AICORE_ACCELERATOR_PROVIDER`,
`AICORE_ACCELERATOR_DEVICE_ID`, `AICORE_ACCELERATOR_PACKAGES`, optional
`AICORE_ACCELERATOR_PROVIDER_LIBRARY`, optional absolute
`AICORE_ACCELERATOR_MODEL_PATH`, and the same `AICORE_ONNX_PYTHON`.
The checked-in `policy_classifier_v1.onnx` exports the existing JSON weights
with consumer feature dimension **4** and output dimension **5**. A queued
session performs bounded, capability-gated model dispatch on the slow side.
The fallback must be session-only; hardware setters are not slow preparation.
Windows composition discovers the selected DXGI device; NPU binding currently
requires one unambiguous PnP NPU and EP ordinal 0. Multiple-NPU identity mapping
fails closed. Queue capacity is 64 and backend concurrency is 1. Five-second
subprocess limits and expiry reject tardy output. Provider-selection capability
is only added after actual session creation. Failed configuration logs a warning
and retains the platform fallback; it does not claim an accelerator execution.

On this host, DirectML inference passed with node-provider evidence. The AMD
WinML NPU EP DLL registers and discovers the NPU. The default FP32 policy
model is CPU-only under that EP and fails closed when CPU fallback is disabled.
`scripts/models/quantize_policy_npu.py` can generate an **experimental** QDQ
candidate under `env/`; when selected explicitly by absolute model path, all
profiled nodes executed on the NPU and the Rust queue returned real inference.
The 1000 deterministic synthetic comparisons against FP32 showed a maximum
score difference of 0.00354925 and 16 top-label differences, so it is not
qualified as a production-equivalent replacement. The FP32 default remains.
The NPU EP session is one request per child process. Once a session exists,
registration is process scoped and exits with that child to avoid vendor DLL
teardown crashes; session creation failures unregister normally. NPU inference
on the experimental candidate is **PASS**, production policy-model NPU
acceptance remains **NOT VERIFIED**. No clock, power or global device setting
is changed. The current Windows adapter has GPU/NPU identity but no observed
utilization on this host; AdaptiveLoop therefore declines device-model
inference instead of treating missing utilization as zero. Direct queue
probe success is not automatic production daemon routing. See the current
destination report for native evidence.

`IntentAgentCoordinator` dispatches up to four bounded providers concurrently.
Only eligible, timely, finite proposals vote; ties fail and consensus expiry
is the earliest contributor expiry. The daemon ensemble excludes cloud
providers by default. It coordinates classification proposals and cannot
execute policies or launch independent autonomous tool agents.

Verification commands (using the existing configured Rust 1.75 environment):

```powershell
$env:AICORE_INTENT_BACKEND='json'
cargo run -p aicore-ai --example configured_intent --locked --offline
$env:AICORE_INTENT_BACKEND='onnx'
$env:AICORE_ONNX_PYTHON='C:\Users\12282\anaconda3\python.exe'
cargo run -p aicore-ai --example configured_intent --locked --offline
./scripts/verify/simulated_platform_ollama_intent.ps1 -Backend ensemble
```

The interpreter path above is this verification host's existing interpreter,
not a portable default. Dependency downloads for this run were confined to
`env/onnx-packages` and `env/pip-cache`; see the destination verification report.
