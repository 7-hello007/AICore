[English](README.md) | [Chinese](README.zh-CN.md)
# AICore — AI Computer Control Layer

AICore is a computer control layer for Windows and Linux. It observes platform capabilities, application tasks and system state; combines them with optional local inference and retained history; proposes resource policies; and, when authorized and supported, applies, verifies and records their effects. Its goal is contextual resource coordination, rather than a collection of fixed optimization switches.

The root [destination](destination.md) defines the product goal and phased design. A design goal, implemented code and platform acceptance are separate claims. The repository contains working Model Mode and simulated-platform paths, Windows and Linux adapters, and many closed-loop components. It does not imply certification of every device, OS or destination goal.

## Control path

```text
Platform capabilities and telemetry → Context / Workload → TierHub / Task
                                    → Prediction / AI proposal → Policy → Validation
                                    → Runtime routing / binding → Execution
                                    → Readback / Verification → Rollback / SafeMode
                                    → Memory / Feedback / Events / Dashboard
```

Core uses shared contracts for context, policy, validation and execution results. Adapters contain platform-specific discovery and control. AI may propose an intent or prediction, but cannot bypass policy validation. The fast control side consumes time-limited policy handoffs; slower telemetry, history and inference work cannot turn an expired or failed result into a successful action. `Success`, `Skipped`, `NotSupported`, `Failed` and `NoOp` have distinct meanings. Model and simulated execution are not evidence of hardware effect.

Application integration has three independent tiers: L0 observes unmodified applications; L1 accepts application-declared task information; L2 uses native task interfaces. IPC is a local, versioned request channel. Plugins enter through trust checks, capability binding and managed process lifecycle. Personal memory is opt-in, subject to retention, and only supports prediction or feedback when evidence is available.

## Repository structure

| Path | Responsibility |
|---|---|
| `contracts/` | Cross-component data, states, errors and traits |
| `core/` | Context, policy, validation, execution, verification and safety logic |
| `runtime/`, `runtime-router/` | Control loops, policy handoff, routing and binding |
| `adapters/windows/`, `adapters/linux/` | OS telemetry, capability discovery and platform control |
| `adapters/simulated-platform/` | In-memory platform simulation using captured fixtures |
| `ai/`, `ai-runtime/`, `models/` | Prediction and intent providers, local runtimes and model assets |
| `api/`, `ipc/` | Application registration/identity, public API and local transport |
| `tier/`, `tier-l0/`, `tier-l1/`, `tier-l2/`, `task-runtime/` | Compatibility tiers and task lifecycle |
| `memory/` | History, personal memory and persistence |
| `plugin/` | Discovery, trust, activation, isolation and capability binding |
| `service-host/` | Service-host code |
| `apps/` | Daemon, CLI, terminal/Web Dashboard and benchmarks |
| `tests/`, `scripts/`, `examples/` | Tests, verification tools and examples |
| `deploy/`, `docs/` | Delivery assets and architecture, contract and acceptance documentation |

See [Cargo.toml](Cargo.toml) for the authoritative Rust workspace membership.

## Build and validation

The declared Rust baseline is 1.75, with dependencies pinned by `Cargo.lock`:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
```

`cargo test --workspace --all-targets --locked` spans the workspace. Some platform tests exercise actual owned processes or power APIs, so review the [test safety guide](docs/quality/DESTINATION_TEST_SAFETY.md) before running a native suite. Report filtered or isolated test results as such.

The daemon selects its execution path with `AICORE_EXECUTION_MODE`:

| Value | Scope |
|---|---|
| `model` | Local model and model-execution path; no hardware-control claim |
| `simulated-platform` | Captured-fixture, in-memory platform simulation |
| `platform` | Native adapter path; may affect host resources and requires target-specific review |

On Windows, `scripts/verify/simulated_platform_windows_smoke.ps1` checks simulated daemon ticks and named-pipe status queries. It is simulation evidence, not native certification. The policy classifier is `models/policy_classifier_v1.json`. Intent inference selects Ollama, local JSON, ONNX or local-provider coordination through `AICORE_INTENT_BACKEND`. The model name and paths are configurable; `AICORE_OLLAMA_MODEL` defaults to `llama3.2:3b`. Unavailable backends fail explicitly rather than pretending a mock inference succeeded. See [intent backends](docs/ai/INTENT_BACKENDS.md).

## Implementation and acceptance

The repository includes policy/execution, rollback and SafeMode, application and task integration, plugin and IPC contracts, personal memory, Dashboard, local models and the platform simulator. [Scheduling and feedback](docs/runtime/SCHEDULING_AND_FEEDBACK.md) describes GPU/NPU admission, pre-task preparation and observed feedback wiring. These components have different levels of integration and acceptance. Unit, mock and simulator results do not certify native GPU/NPU/power control, service installation or cross-vendor hardware behavior.

Use the [destination requirement matrix](docs/quality/DESTINATION_REQUIREMENTS.md) for the implementation inventory, the [verification report](docs/quality/DESTINATION_VERIFICATION_REPORT.md) for completed evidence, and the separate [outstanding tests](docs/quality/OUTSTANDING_TESTS.md) register for acceptance work that remains open. The [documentation index](docs/DOCUMENTATION_INDEX.md) leads to long-term architecture and interface documents. Historical phase reports in `docs/quality/` do not constitute product-wide completion.

## License

Apache License 2.0; see [LICENSE](LICENSE) and [NOTICE](NOTICE).
