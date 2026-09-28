[English](CONFIGURATION.md) | [Chinese](CONFIGURATION.zh-CN.md)
# Configuration

Configuration must support:

- configuration-file loading
- Environment-variable overrides
- Temporary CLI overrides
- Version migration
- Validation
- Safe fallback defaults

Configuration precedence: `CLI > Environment > Config File > Defaults`.
