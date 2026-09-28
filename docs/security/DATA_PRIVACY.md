[English](DATA_PRIVACY.md) | [Chinese](DATA_PRIVACY.zh-CN.md)
# Data Privacy

Default principles: local first, data minimization, and explicit retention limits.

Before data is sent to a model, check:

- whether it contains sensitive user data
- whether the selected model may access it
- whether cloud processing is permitted
- whether the data can be anonymized

All retained history must support deletion.

Currently, `HistoricalRecord.application_id` is populated only from explicitly registered L1, L2, or queued tasks.
When `history_enabled` is false, no history is written. When `profile_enabled` is false, history is not read for
personalization. Application identifiers remain subject to the same retention, deletion, and audit controls as
other personal-behavior data stored in SQLite or JSON. Pattern aggregation requires at least three samples for
the same application and UTC hour. Predictions require an explicit task identity and are never inferred from a
window title.
