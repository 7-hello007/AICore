use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use aicore_contracts::ipc::protocol::{
    Envelope, IpcError, Request, ResponseData, ResponseEnvelope, IPC_PROTOCOL_VERSION,
};
use tokio::sync::{mpsc, OwnedSemaphorePermit, Semaphore};

const CONNECTION_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

use crate::transport::{server_listen, IpcListener, IpcStream, IpcTransport};
use crate::ClientIdentity;

/// 请求处理器。daemon 实现此 trait，把 IPC 请求映射到内部服务。
pub trait IpcHandler: Send + Sync + 'static {
    /// 处理一条请求并返回响应数据；错误通过 `Err` 返回。
    fn handle(&self, request: Request) -> Result<ResponseData, IpcError>;

    /// Transport-authenticated peer path. Existing in-process handlers may
    /// keep using `handle`; security-sensitive handlers override this method.
    fn handle_with_identity(
        &self,
        request: Request,
        _identity: ClientIdentity,
    ) -> Result<ResponseData, IpcError> {
        self.handle(request)
    }

    /// Handler 名称，用于日志。
    fn name(&self) -> &'static str;
}

#[cfg(all(test, target_os = "windows"))]
mod windows_pipe_tests {
    use super::*;
    use crate::IpcClient;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct IdentityStatusHandler {
        saw_current_peer: AtomicBool,
    }

    impl IpcHandler for IdentityStatusHandler {
        fn handle(&self, _request: Request) -> Result<ResponseData, IpcError> {
            Err(IpcError::not_supported("identity required"))
        }

        fn handle_with_identity(
            &self,
            request: Request,
            identity: ClientIdentity,
        ) -> Result<ResponseData, IpcError> {
            if !identity.is_current() {
                return Err(IpcError::new("ERR_PERMISSION_DENIED", "peer not current"));
            }
            self.saw_current_peer.store(true, Ordering::SeqCst);
            match request {
                Request::GetStatus => Ok(ResponseData::Status {
                    status: aicore_contracts::runtime::daemon_status::DaemonStatus::new(1),
                }),
                _ => Err(IpcError::not_supported("unsupported test request")),
            }
        }

        fn name(&self) -> &'static str {
            "windows_identity_status_test"
        }
    }

    #[test]
    fn newer_major_handshake_selects_two_but_standalone_requests_need_session() {
        let handler: Arc<dyn IpcHandler> = Arc::new(IdentityStatusHandler {
            saw_current_peer: AtomicBool::new(false),
        });
        let mut handshake = Envelope::new(
            "negotiate",
            Request::Handshake {
                capabilities: vec![],
                supported_versions: vec![
                    aicore_contracts::schema::SchemaVersion::new(2, 0),
                    IPC_PROTOCOL_VERSION,
                ],
            },
        );
        handshake.version = aicore_contracts::schema::SchemaVersion::new(2, 0);
        let accepted = process_envelope(&handler, handshake, ClientIdentity::Internal);
        assert!(
            matches!(accepted.data, Some(ResponseData::HandshakeAck { selected_version, .. })
            if selected_version == aicore_contracts::ipc::protocol::IPC_PROTOCOL_V2)
        );
        let mut wrong_wire = Envelope::new("wrong", Request::GetStatus);
        wrong_wire.version = aicore_contracts::schema::SchemaVersion::new(2, 0);
        assert_eq!(
            process_envelope(&handler, wrong_wire, ClientIdentity::Internal)
                .error
                .unwrap()
                .code,
            "ERR_PROTOCOL_MISMATCH"
        );
        assert!(
            process_envelope(
                &handler,
                Envelope::new("right", Request::GetStatus),
                ClientIdentity::Internal
            )
            .success
        );
    }

    #[tokio::test]
    async fn named_pipe_handshake_ack_and_peer_identity_roundtrip() {
        let path = format!(
            r"\\.\pipe\aicore-sim-ipc-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let handler = Arc::new(IdentityStatusHandler {
            saw_current_peer: AtomicBool::new(false),
        });
        let (shutdown, rx) = mpsc::channel(1);
        let server = IpcServer::bind(&path, handler.clone())
            .unwrap()
            .with_shutdown(rx);
        let task = tokio::spawn(server.run());
        let mut client = IpcClient::connect(&path).await.unwrap();
        let response = client
            .call(
                "status",
                Request::GetStatus,
                std::time::Duration::from_secs(2),
            )
            .await
            .unwrap();
        assert!(matches!(response.data, Some(ResponseData::Status { .. })));
        assert!(handler.saw_current_peer.load(Ordering::SeqCst));
        let mut v2 = IpcClient::connect_with_versions(
            &path,
            vec![
                aicore_contracts::ipc::protocol::IPC_PROTOCOL_V2,
                IPC_PROTOCOL_VERSION,
            ],
        )
        .await
        .unwrap();
        assert_eq!(v2.negotiated_version().major, 2);
        let response = v2
            .call(
                "v2-status",
                Request::GetStatus,
                std::time::Duration::from_secs(2),
            )
            .await
            .unwrap();
        assert_eq!(response.version.major, 2);
        assert!(matches!(response.data, Some(ResponseData::Status { .. })));
        shutdown.send(()).await.unwrap();
        task.await.unwrap();
    }
}

/// IPC Server。
pub struct IpcServer {
    listener: IpcListener,
    handler: Arc<dyn IpcHandler>,
    shutdown_rx: mpsc::Receiver<()>,
    max_connections: Arc<Semaphore>,
}

impl IpcServer {
    /// 创建 server，绑定到指定路径。
    pub fn bind(path: impl Into<PathBuf>, handler: Arc<dyn IpcHandler>) -> std::io::Result<Self> {
        let listener = server_listen(path.into())?;
        let (_tx, rx) = mpsc::channel(1);
        Ok(Self {
            listener,
            handler,
            shutdown_rx: rx,
            max_connections: Arc::new(Semaphore::new(32)),
        })
    }

    pub fn path(&self) -> &std::path::Path {
        self.listener.path()
    }

    /// 设置 shutdown channel。
    pub fn with_shutdown(mut self, rx: mpsc::Receiver<()>) -> Self {
        self.shutdown_rx = rx;
        self
    }

    /// 启动事件循环。
    pub async fn run(mut self) {
        tracing::info!(
            component = "ipc_server",
            event = "started",
            path = %self.listener.path().display(),
            handler = self.handler.name(),
            "ipc server started"
        );

        loop {
            tokio::select! {
                biased;

                _ = self.shutdown_rx.recv() => {
                    tracing::info!(
                        component = "ipc_server",
                        event = "shutdown",
                        "ipc server shutting down"
                    );
                    break;
                }

                result = self.listener.accept() => {
                    match result {
                        Ok(stream) => {
                            let handler = self.handler.clone();
                            let permits = self.max_connections.clone();
                            let permit = match permits.try_acquire_owned() {
                                Ok(permit) => permit,
                                Err(_) => {
                                    tracing::warn!(component = "ipc_server", event = "connection_rejected", "connection limit reached");
                                    continue;
                                },
                            };
                            tokio::spawn(async move {
                                if let Err(e) = handle_connection(stream, handler, permit).await {
                                    tracing::warn!(
                                        component = "ipc_server",
                                        event = "connection_error",
                                        error = %e,
                                        "ipc connection ended with error"
                                    );
                                }
                            });
                        }
                        Err(e) => {
                            tracing::warn!(
                                component = "ipc_server",
                                event = "accept_failed",
                                error = %e,
                                "ipc accept failed"
                            );
                            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                        }
                    }
                }
            }
        }
    }
}

async fn handle_connection(
    mut stream: IpcStream,
    handler: Arc<dyn IpcHandler>,
    _permit: OwnedSemaphorePermit,
) -> std::io::Result<()> {
    let identity = stream.peer_identity();
    let mut seen = HashSet::<String>::new();
    let mut acknowledgements = false;
    let mut selected_version = IPC_PROTOCOL_VERSION;
    loop {
        let line = match tokio::time::timeout(CONNECTION_IDLE_TIMEOUT, stream.read_message()).await
        {
            Ok(Ok(Some(line))) => line,
            Ok(Ok(None)) | Err(_) => break,
            Ok(Err(error)) => return Err(error),
        };

        let mut send_ack = false;
        let response = match serde_json::from_str::<Envelope>(&line) {
            Ok(envelope) => {
                if envelope.request_id.trim().is_empty() {
                    ResponseEnvelope::err(
                        "unknown",
                        IpcError::invalid_request("request_id must not be empty"),
                    )
                } else if !seen.insert(envelope.request_id.clone()) {
                    ResponseEnvelope::err(
                        envelope.request_id,
                        IpcError::invalid_request("duplicate request_id"),
                    )
                } else if matches!(envelope.payload, Request::Handshake { .. }) {
                    let response = dispatch_blocking(handler.clone(), envelope, identity).await;
                    if let Some(ResponseData::HandshakeAck {
                        selected_version: version,
                        ..
                    }) = &response.data
                    {
                        selected_version = *version;
                    }
                    acknowledgements = matches!(
                        &response.data,
                        Some(ResponseData::HandshakeAck { capabilities, .. })
                            if capabilities.iter().any(|capability| capability == "request_ack")
                    );
                    response
                } else {
                    send_ack = acknowledgements;
                    if envelope.version != selected_version {
                        let mut response = ResponseEnvelope::err(
                            envelope.request_id,
                            IpcError::protocol_mismatch(envelope.version, selected_version),
                        );
                        response.version = selected_version;
                        response
                    } else {
                        // Both supported wire dialects have the same typed
                        // payload, while V2 is accepted only on this session.
                        let mut canonical = envelope;
                        canonical.version = IPC_PROTOCOL_VERSION;
                        let mut response =
                            dispatch_blocking(handler.clone(), canonical, identity).await;
                        response.version = selected_version;
                        response
                    }
                }
            },
            Err(e) => ResponseEnvelope::err(
                "unknown",
                IpcError::invalid_request(format!("cannot parse envelope: {}", e)),
            ),
        };

        if send_ack {
            let mut ack = ResponseEnvelope::ok(response.request_id.clone(), ResponseData::Ack);
            ack.version = selected_version;
            write_response(&mut stream, &ack).await?;
        }
        write_response(&mut stream, &response).await?;
        if seen.len() >= 4096 {
            break;
        }
    }
    Ok(())
}

async fn dispatch_blocking(
    handler: Arc<dyn IpcHandler>,
    envelope: Envelope,
    identity: ClientIdentity,
) -> ResponseEnvelope {
    let request_id = envelope.request_id.clone();
    tokio::task::spawn_blocking(move || process_envelope(&handler, envelope, identity))
        .await
        .unwrap_or_else(|error| {
            ResponseEnvelope::err(request_id, IpcError::internal(error.to_string()))
        })
}

async fn write_response(
    stream: &mut IpcStream,
    response: &ResponseEnvelope,
) -> std::io::Result<()> {
    let json = serde_json::to_string(response).unwrap_or_else(|_| {
            r#"{"version":"1.0","request_id":"unknown","success":false,"error":{"code":"ERR_INTERNAL","message":"serialize failed"}}"#.to_string()
        });
    stream.write_message(&json).await
}

fn process_envelope(
    handler: &Arc<dyn IpcHandler>,
    envelope: Envelope,
    identity: ClientIdentity,
) -> ResponseEnvelope {
    let negotiable_handshake = matches!(&envelope.payload,
        Request::Handshake { supported_versions, .. }
            if aicore_contracts::ipc::protocol::negotiate_version(supported_versions).is_some());
    if envelope.version != IPC_PROTOCOL_VERSION && !negotiable_handshake {
        return ResponseEnvelope::err(
            envelope.request_id,
            IpcError::protocol_mismatch(envelope.version, IPC_PROTOCOL_VERSION),
        );
    }

    let request_id = envelope.request_id.clone();
    if matches!(
        &envelope.payload,
        Request::Register { .. }
            | Request::Unregister { .. }
            | Request::SubmitTask { .. }
            | Request::DeclareCurrentTask { .. }
            | Request::ClearCurrentTask { .. }
            | Request::ProposeIntent { .. }
            | Request::PrepareUpcomingTask { .. }
            | Request::SubmitNativeTask { .. }
            | Request::GetNativeTask { .. }
            | Request::CancelNativeTask { .. }
    ) && !identity.is_current()
    {
        return ResponseEnvelope::err(
            request_id,
            IpcError::new(
                "ERR_PERMISSION_DENIED",
                "IPC peer identity is no longer current",
            ),
        );
    }
    match envelope.payload {
        Request::Handshake {
            capabilities,
            supported_versions,
        } => {
            let offered = if supported_versions.is_empty() {
                vec![envelope.version]
            } else {
                supported_versions
            };
            let Some(selected_version) =
                aicore_contracts::ipc::protocol::negotiate_version(&offered)
            else {
                return ResponseEnvelope::err(
                    request_id,
                    IpcError::protocol_mismatch(envelope.version, IPC_PROTOCOL_VERSION),
                );
            };
            let accepted = capabilities
                .into_iter()
                .filter(|capability| capability == "request_ack")
                .collect();
            let mut response = ResponseEnvelope::ok(
                request_id,
                ResponseData::HandshakeAck {
                    capabilities: accepted,
                    selected_version,
                },
            );
            response.version = selected_version;
            response
        },
        request => match handler.handle_with_identity(request, identity) {
            Ok(data) => ResponseEnvelope::ok(request_id, data),
            Err(e) => ResponseEnvelope::err(request_id, e),
        },
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use crate::transport::client_connect;

    struct StatusHandler;

    impl IpcHandler for StatusHandler {
        fn handle(&self, request: Request) -> Result<ResponseData, IpcError> {
            match request {
                Request::GetStatus => Ok(ResponseData::Status {
                    status: aicore_contracts::runtime::daemon_status::DaemonStatus::new(1),
                }),
                _ => Err(IpcError::not_supported("unsupported test request")),
            }
        }

        fn name(&self) -> &'static str {
            "test"
        }
    }

    #[tokio::test]
    async fn negotiated_connection_acks_before_response() {
        let path = std::env::temp_dir().join(format!(
            "aicore-ipc-handshake-{}-{}.sock",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let (shutdown, rx) = mpsc::channel(1);
        let server = IpcServer::bind(&path, Arc::new(StatusHandler))
            .unwrap()
            .with_shutdown(rx);
        let task = tokio::spawn(server.run());
        let mut stream = client_connect(&path).await.unwrap();

        let handshake = Envelope::new(
            "handshake",
            Request::Handshake {
                capabilities: vec!["request_ack".to_string()],
                supported_versions: vec![IPC_PROTOCOL_VERSION],
            },
        );
        stream
            .write_message(&serde_json::to_string(&handshake).unwrap())
            .await
            .unwrap();
        let response: ResponseEnvelope =
            serde_json::from_str(&stream.read_message().await.unwrap().unwrap()).unwrap();
        assert!(matches!(
            response.data,
            Some(ResponseData::HandshakeAck { capabilities, selected_version })
                if capabilities == ["request_ack"] && selected_version == IPC_PROTOCOL_VERSION
        ));

        stream
            .write_message(
                &serde_json::to_string(&Envelope::new("status", Request::GetStatus)).unwrap(),
            )
            .await
            .unwrap();
        let ack: ResponseEnvelope =
            serde_json::from_str(&stream.read_message().await.unwrap().unwrap()).unwrap();
        assert_eq!(ack.request_id, "status");
        assert!(matches!(ack.data, Some(ResponseData::Ack)));
        let final_response: ResponseEnvelope =
            serde_json::from_str(&stream.read_message().await.unwrap().unwrap()).unwrap();
        assert_eq!(final_response.request_id, "status");
        assert!(matches!(
            final_response.data,
            Some(ResponseData::Status { .. })
        ));

        stream
            .write_message(
                &serde_json::to_string(&Envelope::new("status", Request::GetStatus)).unwrap(),
            )
            .await
            .unwrap();
        let duplicate_error: ResponseEnvelope =
            serde_json::from_str(&stream.read_message().await.unwrap().unwrap()).unwrap();
        assert!(!duplicate_error.success);
        assert_eq!(duplicate_error.request_id, "status");

        shutdown.send(()).await.unwrap();
        task.await.unwrap();
    }

    #[tokio::test]
    async fn client_without_handshake_gets_one_response() {
        let path = std::env::temp_dir().join(format!(
            "aicore-ipc-legacy-{}-{}.sock",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let (shutdown, rx) = mpsc::channel(1);
        let server = IpcServer::bind(&path, Arc::new(StatusHandler))
            .unwrap()
            .with_shutdown(rx);
        let task = tokio::spawn(server.run());
        let mut stream = client_connect(&path).await.unwrap();
        stream
            .write_message(
                &serde_json::to_string(&Envelope::new("legacy", Request::GetStatus)).unwrap(),
            )
            .await
            .unwrap();
        let response: ResponseEnvelope =
            serde_json::from_str(&stream.read_message().await.unwrap().unwrap()).unwrap();
        assert!(matches!(response.data, Some(ResponseData::Status { .. })));
        assert_eq!(response.request_id, "legacy");
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(20), stream.read_message())
                .await
                .is_err()
        );
        shutdown.send(()).await.unwrap();
        task.await.unwrap();
    }

    #[test]
    fn app_request_rechecks_executable_identity_before_handler() {
        use std::os::unix::fs::MetadataExt;
        let pid = std::process::id() as i32;
        let uid = std::fs::metadata(format!("/proc/{pid}")).unwrap().uid();
        let start_time_ticks = crate::transport::read_process_start_time(pid).unwrap();
        let (executable_dev, executable_ino) =
            crate::transport::read_process_executable_identity(pid).unwrap();
        let replaced = ClientIdentity::Linux {
            uid,
            pid,
            start_time_ticks,
            executable_dev,
            executable_ino: executable_ino.wrapping_add(1),
        };
        let handler: Arc<dyn IpcHandler> = Arc::new(StatusHandler);
        let request = Envelope::new(
            "register-replaced",
            Request::Register {
                application_id: aicore_contracts::application::id::ApplicationId::new("test.app"),
                name: "test".into(),
                version: "1.0".into(),
                capabilities: Vec::new(),
            },
        );
        let result = process_envelope(&handler, request, replaced);
        assert!(!result.success);
        assert_eq!(result.error.unwrap().code, "ERR_PERMISSION_DENIED");
    }

    #[test]
    fn incompatible_protocol_version_is_rejected_before_handler() {
        let handler: Arc<dyn IpcHandler> = Arc::new(StatusHandler);
        let mut envelope = Envelope::new("old-version", Request::GetStatus);
        envelope.version = aicore_contracts::schema::SchemaVersion::new(2, 0);
        let result = process_envelope(&handler, envelope, ClientIdentity::Internal);
        assert!(!result.success);
        assert_eq!(result.request_id, "old-version");
        assert_eq!(result.error.unwrap().code, "ERR_PROTOCOL_MISMATCH");
    }

    #[test]
    fn handshake_negotiates_explicit_versions_and_rejects_unknown_major() {
        let handler: Arc<dyn IpcHandler> = Arc::new(StatusHandler);
        let mut envelope = Envelope::new(
            "future",
            Request::Handshake {
                capabilities: vec!["request_ack".into()],
                supported_versions: vec![IPC_PROTOCOL_VERSION],
            },
        );
        envelope.version = aicore_contracts::schema::SchemaVersion::new(2, 0);
        let response = process_envelope(&handler, envelope, ClientIdentity::Internal);
        assert!(response.success);
        assert_eq!(response.version, IPC_PROTOCOL_VERSION);
        let unsupported = Envelope {
            version: aicore_contracts::schema::SchemaVersion::new(3, 0),
            request_id: "unsupported".into(),
            timestamp_unix_ms: 1,
            payload: Request::Handshake {
                capabilities: vec![],
                supported_versions: vec![aicore_contracts::schema::SchemaVersion::new(3, 0)],
            },
        };
        assert_eq!(
            process_envelope(&handler, unsupported, ClientIdentity::Internal)
                .error
                .unwrap()
                .code,
            "ERR_PROTOCOL_MISMATCH"
        );
    }
}
