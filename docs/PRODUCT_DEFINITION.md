[English](PRODUCT_DEFINITION.md) | [Chinese](PRODUCT_DEFINITION.zh-CN.md)
# Product Definition

## Product positioning

AICore is an **AI Computer Control Layer** between applications and operating-system or hardware interfaces. It combines context understanding, prediction, policy, and resource coordination so existing computers can evolve toward AI-managed operation.

## What AICore is not

| Not AICore | Reason |
|---|---|
| New operating system | AICore relies on and augments an existing OS; it does not replace the OS. |
| Conventional PC optimizer | Its core is contextual, closed-loop control rather than fixed cleanup or acceleration rules. |
| Chatbot | AI is used to understand intent and system state, not primarily for conversation. |
| Hardware driver collection | Drivers and vendor SDKs remain behind adapter and runtime boundaries. |
| Mandatory application SDK | The L0 compatibility path must work for unchanged applications. |

## Product value

1. Reduce the need for users to maintain many performance, power, and background-task rules manually.
2. Give traditional software system-level AI adaptation without requiring code changes.
3. Let participating applications adopt richer AICore capabilities incrementally.
4. Provide one control abstraction across operating systems, CPU/GPU/NPU resources, and AI runtimes.
