[English](ARCHITECTURE_RULES.md) | [Chinese](ARCHITECTURE_RULES.zh-CN.md)
# Architecture Rules

| Rule | Requirement |
|---|---|
| R-01 | Core does not depend on a specific platform |
| R-02 | Core does not depend on UI |
| R-03 | Core does not depend on a specific AI model |
| R-04 | Tiers do not depend on one another |
| R-05 | Adapters implement contracts and do not make policy decisions |
| R-06 | AI does not directly perform system operations |
| R-07 | Every policy must pass through the Validator |
| R-08 | Every system-control operation must pass through an Executor |
| R-09 | Cross-module state is exchanged through contracts, events, and DTOs |
| R-10 | New functionality gives priority to adding modules/plugins instead of modifying Core |
| R-11 | Runtime failure must be reversible |
| R-12 | Any tier or plugin can be removed without breaking the others |
