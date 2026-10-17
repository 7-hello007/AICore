//! Dashboard IPC channel integration test.

#[cfg(target_os = "linux")]
mod linux {
    use std::sync::Arc;
    use std::time::Duration;

    use aicore_contracts::{
        capability::entry::CapabilityEntry,
        ipc::protocol::{IpcError, Request, ResponseData},
        runtime::daemon_status::DaemonStatus,
    };
    use aicore_dashboard::DashboardClient;
    use aicore_ipc::{IpcHandler, IpcServer};

    #[derive(Clone)]
    struct MockHandler;

    impl IpcHandler for MockHandler {
        fn process_spec(&self) -> Option<aicore_ipc::process_handler::ProcessHandlerSpec> {
            Some(aicore_ipc::process_handler::ProcessHandlerSpec {
                process: aicore_contracts::plugin::host::PluginProcessSpec { plugin_id:"dashboard-fixture".into(), artifact_path:"/usr/bin/python3".into(), timeout_ms:1000, memory_limit_mb:64 },
                args:vec!["-c".into(),"import sys,json; print(json.dumps(json.loads(sys.stdin.readline())['state']),flush=True)".into()],
            })
        }
        fn prepare_process_state(
            &self,
            envelope: &aicore_contracts::ipc::protocol::Envelope,
            peer: aicore_ipc::ClientIdentity,
            _: std::time::Instant,
        ) -> Result<serde_json::Value, IpcError> {
            // Finite data fixture only. Production daemon preparation is tested
            // by the actual aicored executable, not this snapshot echo worker.
            let handler: Arc<dyn IpcHandler> = Arc::new(self.clone());
            serde_json::to_value(aicore_ipc::server::process_envelope(
                &handler,
                envelope.clone(),
                peer,
            ))
            .map_err(|e| IpcError::internal(e.to_string()))
        }
        fn handle(&self, request: Request) -> Result<ResponseData, IpcError> {
            match request {
                Request::GetStatus => Ok(ResponseData::Status {
                    status: {
                        let mut s = DaemonStatus::new(999);
                        s.tick_count = 5;
                        s
                    },
                }),
                Request::GetCapabilities => Ok(ResponseData::Capabilities {
                    component: "test".to_string(),
                    entries: vec![CapabilityEntry::supported("cpu.telemetry")],
                }),
                Request::GetRecentTicks { .. } => Ok(ResponseData::RecentTicks { ticks: vec![] }),
                Request::ListPlugins => Ok(ResponseData::Plugins { entries: vec![] }),
                _ => Err(IpcError::not_supported("n/a")),
            }
        }
        fn name(&self) -> &'static str {
            "mock"
        }
    }

    async fn setup(
        socket: std::path::PathBuf,
    ) -> (tokio::task::JoinHandle<()>, tokio::sync::mpsc::Sender<()>) {
        let handler: Arc<dyn IpcHandler> = Arc::new(MockHandler);
        let (tx, rx) = tokio::sync::mpsc::channel::<()>(1);
        let server = IpcServer::bind(socket, handler)
            .expect("bind")
            .with_shutdown(rx);
        let handle = tokio::spawn(async move {
            server.run().await;
        });
        (handle, tx)
    }

    #[tokio::test]
    async fn fetch_populates_all_panels() {
        let socket =
            std::env::temp_dir().join(format!("aicore-dashboard-test-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&socket);

        let (handle, shutdown) = setup(socket.clone()).await;
        tokio::time::sleep(Duration::from_millis(50)).await;

        let client = DashboardClient::new(socket.clone());
        let snap = client.fetch().await;

        assert!(snap.errors.is_empty(), "errors: {:?}", snap.errors);
        assert!(snap.daemon_status.is_some());
        assert_eq!(snap.daemon_status.as_ref().unwrap().pid, 999);
        assert_eq!(snap.capabilities.len(), 1);
        assert_eq!(snap.capabilities[0].name, "cpu.telemetry");
        assert!(snap.recent_ticks.is_empty());
        assert!(snap.plugin_states.is_empty());

        let _ = shutdown.send(()).await;
        let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
        let _ = std::fs::remove_file(&socket);
    }

    #[tokio::test]
    async fn fetch_without_server_records_error() {
        let socket = std::env::temp_dir().join(format!(
            "aicore-dashboard-noserver-{}.sock",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&socket);

        let client = DashboardClient::new(socket);
        let snap = client.fetch().await;

        assert!(snap.daemon_status.is_none());
        assert!(!snap.errors.is_empty());
        assert!(snap.errors[0].contains("connect"));
    }

    #[tokio::test]
    async fn fetch_respects_ticks_limit() {
        let socket = std::env::temp_dir().join(format!(
            "aicore-dashboard-limit-{}.sock",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&socket);

        let (handle, shutdown) = setup(socket.clone()).await;
        tokio::time::sleep(Duration::from_millis(50)).await;

        let client = DashboardClient::new(socket.clone()).with_recent_ticks_limit(3);
        assert_eq!(client.recent_ticks_limit(), 3);

        let _snap = client.fetch().await;

        let _ = shutdown.send(()).await;
        let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
        let _ = std::fs::remove_file(&socket);
    }
}
