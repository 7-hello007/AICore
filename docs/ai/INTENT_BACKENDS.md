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
| `onnx` | CPU ONNX Runtime subprocess via `PythonOnnxSessionFactory` | Explicit `AICORE_ONNX_PYTHON`; packages in project `env/onnx-packages` |
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
only project-local packages, requests `CPUExecutionProvider`, validates real
model schemas, drains stdout/stderr, bounds protocol bytes and kills timed-out
subprocesses. It does not install, download, select GPU/NPU, or silently fall
back to a mock. Sessions are loaded in each isolated subprocess, so this is a
functional CPU adapter, not an optimized persistent serving backend.

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
