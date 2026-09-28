[English](README.md) | [Chinese](README.zh-CN.md)
# AICore documentation

This directory contains the product architecture, contracts, engineering rules and verification evidence. The [root README](../README.md) introduces the repository; [destination](../destination.md) defines the broader product design. Architecture documents describe intended boundaries. For current implementation claims, use [engineering status](quality/ENGINEERING_STATUS.md) and the [destination requirement matrix](quality/DESTINATION_REQUIREMENTS.md). Completed evidence is in the [verification report](quality/DESTINATION_VERIFICATION_REPORT.md); outstanding tests are maintained separately in [outstanding tests](quality/OUTSTANDING_TESTS.md).

The English `.md` file is the primary version of each document. Its `.zh-CN.md` companion is the Simplified Chinese version. Links at the top of each page switch languages; links in Chinese pages point to Chinese companions when available. Code, commands, model identifiers and status words retain their original spelling in both languages.

## Architectural rules

1. Core does not depend on an OS, hardware vendor, AI model or UI.
2. Adapters and runtime providers contain platform-specific behavior.
3. L0, L1 and L2 are independent integration tiers using shared contracts.
4. AI proposes; policy validation and execution decide whether an action occurs.
5. Slow observation and inference hand off time-limited policy to the fast control side.
6. Failures must degrade safely and must not destabilize the OS.
7. Public contracts, events and IPC must be versioned, testable and traceable.

## Document map

| Area | Contents |
|---|---|
| `architecture/` | Control, data and adapter planes; architectural decisions; kernel-necessity analysis |
| `contracts/` | API, data model, events and IPC |
| `context/` | Context, workload and L0/L1/L2 compatibility |
| `ai/` | Providers, inference policy, intent backends and model lifecycle |
| `policy/`, `runtime/` | Policy, routing, execution, feedback and recovery |
| `platform/` | Windows/Linux adapters, capability matrix and simulator |
| `memory/`, `plugin/` | Retention/privacy, plugin trust and lifecycle |
| `ui/`, `security/` | Dashboard and security boundaries |
| `operations/` | Configuration, observability, CI and delivery |
| `quality/` | Acceptance model, completed evidence, outstanding tests and historical phase reports |
| `governance/` | Coding, documentation and decision rules |

Use the [documentation index](DOCUMENTATION_INDEX.md) for individual pages. Historical V3.2/V4 reports remain available as dated evidence; they are not a declaration that the full destination product is accepted.
