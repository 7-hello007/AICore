use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use aicore_contracts::ipc::protocol::{Request, ResponseData};
use aicore_ipc::{unwrap_response, IpcClient};

use crate::snapshot::DashboardSnapshot;

/// Dashboard IPC 客户端。
///
/// V3.2：
/// - 优先走聚合端点 `GetDashboardSnapshot`（一次拉取全字段）。
/// - 聚合端点失败时回退到分端点轮询（V3-L 兼容）。
pub struct DashboardClient {
    ipc_path: PathBuf,
    timeout: Duration,
    /// 仅影响 legacy fallback 路径；聚合端点固定返回 10 条。
    recent_ticks_limit: usize,
}

impl DashboardClient {
    pub fn new(ipc_path: impl Into<PathBuf>) -> Self {
        Self {
            ipc_path: ipc_path.into(),
            timeout: Duration::from_secs(2),
            recent_ticks_limit: 10,
        }
    }

    pub fn with_timeout(mut self, d: Duration) -> Self {
        self.timeout = d;
        self
    }

    pub fn ipc_path(&self) -> &Path {
        &self.ipc_path
    }

    /// 设置 legacy fallback 的 recent ticks 上限（1..=200）。
    pub fn with_recent_ticks_limit(mut self, n: usize) -> Self {
        self.recent_ticks_limit = n.clamp(1, 200);
        self
    }

    pub fn recent_ticks_limit(&self) -> usize {
        self.recent_ticks_limit
    }

    /// 拉取一次聚合快照。
    pub async fn fetch(&self) -> DashboardSnapshot {
        let mut client = match IpcClient::connect(self.ipc_path.clone()).await {
            Ok(c) => c,
            Err(e) => {
                let mut snap = DashboardSnapshot::new(now_unix_secs());
                snap.errors.push(format!("connect: {}", e));
                return snap;
            },
        };

        // 1. 优先聚合端点
        let request_id = format!("dashboard-{}", now_unix_nanos());
        if let Ok(resp) = client
            .call(&request_id, Request::GetDashboardSnapshot, self.timeout)
            .await
        {
            if let Ok(ResponseData::DashboardSnapshot { snapshot }) = unwrap_response(resp) {
                return *snapshot;
            }
        }

        // 2. 回退：分端点
        self.fetch_legacy(&mut client).await
    }

    async fn fetch_legacy(&self, client: &mut IpcClient) -> DashboardSnapshot {
        let mut snap = DashboardSnapshot::new(now_unix_secs());

        match call(client, Request::GetStatus, self.timeout).await {
            Ok(ResponseData::Status { status }) => snap.daemon_status = Some(status),
            Ok(_) => snap
                .errors
                .push("GetStatus: unexpected response".to_string()),
            Err(e) => snap.errors.push(format!("GetStatus: {}", e)),
        }

        match call(client, Request::GetCapabilities, self.timeout).await {
            Ok(ResponseData::Capabilities { entries, .. }) => snap.capabilities = entries,
            Ok(_) => snap
                .errors
                .push("GetCapabilities: unexpected response".to_string()),
            Err(e) => snap.errors.push(format!("GetCapabilities: {}", e)),
        }

        match call(client, Request::GetRecentTicks { limit: 10 }, self.timeout).await {
            Ok(ResponseData::RecentTicks { ticks }) => snap.recent_ticks = ticks,
            Ok(_) => snap
                .errors
                .push("GetRecentTicks: unexpected response".to_string()),
            Err(e) => snap.errors.push(format!("GetRecentTicks: {}", e)),
        }

        match call(client, Request::ListPlugins, self.timeout).await {
            Ok(ResponseData::Plugins { entries }) => snap.plugin_states = entries,
            Ok(_) => snap
                .errors
                .push("ListPlugins: unexpected response".to_string()),
            Err(e) => snap.errors.push(format!("ListPlugins: {}", e)),
        }

        snap
    }
}

async fn call(
    client: &mut IpcClient,
    request: Request,
    timeout: Duration,
) -> Result<ResponseData, String> {
    let request_id = format!("dashboard-{}", now_unix_nanos());
    let resp = client
        .call(&request_id, request, timeout)
        .await
        .map_err(|e| format!("transport: {}", e))?;
    unwrap_response(resp).map_err(|e| format!("{}: {}", e.code, e.message))
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn now_unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}
