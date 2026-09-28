[English](ENGINEERING_STATUS.md) | [Chinese](ENGINEERING_STATUS.zh-CN.md)
# Engineering status

## Scope and decision

AICore `0.4.0` retains the V4 defect-correction code baseline while destination work continues on the same architecture. The current desktop checkout has a **PASS** code gate at the scopes recorded in the [verification report](DESTINATION_VERIFICATION_REPORT.md). Destination-wide product acceptance remains **NOT VERIFIED**. This distinction applies to model, simulated and native evidence alike.

The code keeps policy validation ahead of state-changing binding, preserves task expiry and execution-status semantics, and provides rollback/SafeMode paths. Current integration also includes bounded fast/slow handoff, version-negotiated IPC, application identity checks, plugin trust and typed capability contracts, opt-in memory, feedback, and configurable local intent/model providers. Platform-specific behavior remains in adapters and provider implementations.

## Evidence handling

The completed commands and measurements, their actual scopes, and any restoration results are recorded in the [verification report](DESTINATION_VERIFICATION_REPORT.md). The separate [outstanding tests](OUTSTANDING_TESTS.md) file holds every current incomplete acceptance check and its reason. Historical V4 and destination report sections remain dated evidence; they must not be read as a fresh test run or full product certification.

This documentation-and-source-language round makes no hardware-control or host-configuration change. It verifies source preservation, bilingual navigation and safe workspace build/static gates; it does not rerun native-control tests.
