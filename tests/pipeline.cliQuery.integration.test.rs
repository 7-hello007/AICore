//! IPC server + GetRecentTicks integration test.

#[cfg(target_os = "linux")]
mod linux_tests {
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use aicore_api::ApplicationRegistry;
    use aicore_contracts::{
        context::workload::WorkloadType,
        execution::status::ExecutionOverallStatus,
        ipc::protocol::{Request, ResponseData},
        policy::priority::PolicySource,
        runtime::{
            daemon_status::{DaemonStatus, TickSummary},
            mode::RuntimeMode,
            tick::TickOutcome,
        },
    };
    use aicore_ipc::{unwrap_response, IpcClient, IpcHandler, IpcServer};
    use std::collections::VecDeque;

    /// Simplify TickHistory: Equivalence with daemon.
    struct TestHistory {
        max: usize,
        buf: Mutex<VecDeque<TickSummary>>,
    }

    impl TestHistory {
        fn new(max: usize) -> Self {
            Self {
                max,
                buf: Mutex::new(VecDeque::new()),
            }
        }
        fn append(&self, outcome: &TickOutcome) {
            let s = TickSummary::from_outcome(outcome);
            let mut buf = self.buf.lock().unwrap();
            while buf.len() >= self.max {
                buf.pop_front();
            }
            buf.push_back(s);
        }
        fn snapshot(&self, limit: usize) -> Vec<TickSummary> {
            let buf = self.buf.lock().unwrap();
            let n = limit.min(buf.len());
            let start = buf.len().saturating_sub(n);
            buf.iter().skip(start).cloned().collect()
        }
    }

    struct TestHandler {
        registry: Arc<ApplicationRegistry>,
        history: Arc<TestHistory>,
    }

    impl IpcHandler for TestHandler {
        fn handle(
            &self,
            request: Request,
        ) -> Result<ResponseData, aicore_contracts::ipc::protocol::IpcError> {
            match request {
                Request::GetRecentTicks { limit } => Ok(ResponseData::RecentTicks {
                    ticks: self.history.snapshot(limit),
                }),
                Request::GetStatus => {
                    let _ = &self.registry;
                    Ok(ResponseData::Status {
                        status: DaemonStatus::new(std::process::id()),
                    })
                },
                _ => Err(aicore_contracts::ipc::protocol::IpcError::not_supported(
                    "n/a",
                )),
            }
        }

        fn name(&self) -> &'static str {
            "test_history_handler"
        }
    }

    fn sample_tick(id: u64) -> TickOutcome {
        TickOutcome {
            tick_id: id,
            trace_id: format!("trace-{}-100", id),
            timestamp_unix_ms: 1_700_000_000 + id,
            duration_ms: 5,
            success: true,
            mode_after: RuntimeMode::Normal,
            workload_type: WorkloadType::Development,
            policy_id: Some(format!("policy-{}", id)),
            policy_source: Some(PolicySource::RuleBased),
            validation_accepted: true,
            execution_status: Some(ExecutionOverallStatus::AllSucceeded),
            message: "ok".to_string(),
            predicted_workload: Some(WorkloadType::Development),
            prediction_confidence: Some(0.7),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn get_recent_ticks_returns_appended() {
        let socket =
            std::env::temp_dir().join(format!("aicore-test-ticks-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&socket);

        let history = Arc::new(TestHistory::new(10));
        for i in 1..=3 {
            history.append(&sample_tick(i));
        }

        let registry = Arc::new(ApplicationRegistry::new());
        let handler: Arc<dyn IpcHandler> = Arc::new(TestHandler {
            registry,
            history: history.clone(),
        });

        let (_tx, rx) = tokio::sync::mpsc::channel::<()>(1);
        let server = IpcServer::bind(socket.clone(), handler)
            .expect("bind")
            .with_shutdown(rx);
        let handle = tokio::spawn(async move {
            server.run().await;
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        let mut client = IpcClient::connect(socket.clone()).await.expect("connect");
        let resp = client
            .call(
                "req-1",
                Request::GetRecentTicks { limit: 10 },
                Duration::from_secs(2),
            )
            .await
            .expect("call");

        let data = unwrap_response(resp).expect("ok");
        match data {
            ResponseData::RecentTicks { ticks } => {
                assert_eq!(ticks.len(), 3);
                assert_eq!(ticks[0].tick_id, 1);
                assert_eq!(ticks[2].tick_id, 3);
                assert_eq!(ticks[0].predicted_workload, Some(WorkloadType::Development));
            },
            _ => panic!("unexpected data"),
        }

        let _ = std::fs::remove_file(&socket);
        handle.abort();
    }

    #[tokio::test]
    async fn get_recent_ticks_respects_limit() {
        let socket = std::env::temp_dir().join(format!(
            "aicore-test-ticks-limit-{}.sock",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&socket);

        let history = Arc::new(TestHistory::new(50));
        for i in 1..=20 {
            history.append(&sample_tick(i));
        }

        let registry = Arc::new(ApplicationRegistry::new());
        let handler: Arc<dyn IpcHandler> = Arc::new(TestHandler { registry, history });

        let (_tx, rx) = tokio::sync::mpsc::channel::<()>(1);
        let server = IpcServer::bind(socket.clone(), handler)
            .expect("bind")
            .with_shutdown(rx);
        let handle = tokio::spawn(async move {
            server.run().await;
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        let mut client = IpcClient::connect(socket.clone()).await.expect("connect");
        let resp = client
            .call(
                "req-1",
                Request::GetRecentTicks { limit: 5 },
                Duration::from_secs(2),
            )
            .await
            .expect("call");

        let data = unwrap_response(resp).expect("ok");
        match data {
            ResponseData::RecentTicks { ticks } => {
                assert_eq!(ticks.len(), 5);
                // Recent 5 should read 16.=20
                assert_eq!(ticks[0].tick_id, 16);
                assert_eq!(ticks[4].tick_id, 20);
            },
            _ => panic!("unexpected data"),
        }

        let _ = std::fs::remove_file(&socket);
        handle.abort();
    }

    #[tokio::test]
    async fn empty_history_returns_empty_vec() {
        let socket = std::env::temp_dir().join(format!(
            "aicore-test-ticks-empty-{}.sock",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&socket);

        let history = Arc::new(TestHistory::new(10));
        let registry = Arc::new(ApplicationRegistry::new());
        let handler: Arc<dyn IpcHandler> = Arc::new(TestHandler { registry, history });

        let (_tx, rx) = tokio::sync::mpsc::channel::<()>(1);
        let server = IpcServer::bind(socket.clone(), handler)
            .expect("bind")
            .with_shutdown(rx);
        let handle = tokio::spawn(async move {
            server.run().await;
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        let mut client = IpcClient::connect(socket.clone()).await.expect("connect");
        let resp = client
            .call(
                "req-1",
                Request::GetRecentTicks { limit: 10 },
                Duration::from_secs(2),
            )
            .await
            .expect("call");

        let data = unwrap_response(resp).expect("ok");
        match data {
            ResponseData::RecentTicks { ticks } => {
                assert!(ticks.is_empty());
            },
            _ => panic!("unexpected data"),
        }

        let _ = std::fs::remove_file(&socket);
        handle.abort();
    }

    // Unused Report Warning
    #[allow(dead_code)]
    fn _unused(_: PathBuf) {}
}
