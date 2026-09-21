# Dashboard Data Flow

Realtime 数据：State/Event → Dashboard Snapshot → Terminal/Web。

Historical 数据：Memory → Query → History Views。

Web 使用受控本地 API；Terminal 与 Web 都必须能显示：状态、资源、当前 Context、当前 Policy、Execution Result、Tier、Plugin、Runtime Health。
