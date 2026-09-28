[English](ADAPTER_RULES.md) | [Chinese](ADAPTER_RULES.zh-CN.md)
# Adapter Rules

1. An adapter does not create policies.
2. An adapter does not persist user profiles.
3. An adapter does not read the UI.
4. An adapter returns state, capabilities, or execution results only through contracts.
5. An unsupported capability must return clear `NotSupported/CapabilityUnavailable` status; it cannot be presented as success.
6. Every adapter must pass the same contract-test suite.
