use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use aicore_contracts::ipc::protocol::{
    Envelope, IpcError, Request, ResponseData, ResponseEnvelope, IPC_PROTOCOL_VERSION,
};
use tokio::sync::{mpsc, OwnedSemaphorePermit, Semaphore};

const CONNECTION_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
const HANDLER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

use crate::transport::{server_listen, IpcListener, IpcStream, IpcTransport};
use crate::ClientIdentity;

/// Request processor implemented by the daemon to map IPC requests to internal services.
pub trait IpcHandler: Send + Sync + 'static {
    /// An explicitly configured worker executable. It receives one private
    /// ProcessHandlerRequest on stdin and returns one ResponseEnvelope.
    fn process_spec(&self) -> Option<crate::process_handler::ProcessHandlerSpec> {
        None
    }
    /// Trusted state access only: implementations must use nonblocking locks,
    /// bounded data and no IO/inference or arbitrary synchronous callbacks.
    /// Heavy work belongs to the disposable process, never these broker steps.
    fn prepare_process_state(
        &self,
        _: &Envelope,
        _: ClientIdentity,
        _: std::time::Instant,
    ) -> Result<serde_json::Value, IpcError> {
        Ok(serde_json::Value::Null)
    }
    fn commit_process_response(
        &self,
        envelope: &Envelope,
        _: ClientIdentity,
        _: &serde_json::Value,
        response: serde_json::Value,
        _: std::time::Instant,
    ) -> Result<ResponseEnvelope, IpcError> {
        if !crate::process_handler::response_matches(envelope, &response) {
            return Err(IpcError::invalid_request(
                "invalid worker response identity",
            ));
        }
        serde_json::from_value(response).map_err(|e| IpcError::internal(e.to_string()))
    }
    #[cfg(test)]
    fn inline_test_fixture(&self) -> bool {
        false
    }
    /// Process a request and return response data; error returned via `Err`.
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

    /// Handler name for log.
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
        fn inline_test_fixture(&self) -> bool {
            true
        }
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

    #[tokio::test]
    async fn p1_shutdown_closes_existing_sessions() {
        let path = format!(
            r"\\.\pipe\aicore-p1-shutdown-{}-{}",
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
        let server = IpcServer::bind(&path, handler).unwrap().with_shutdown(rx);
        let task = tokio::spawn(server.run());
        let mut client = IpcClient::connect(&path).await.unwrap();
        shutdown.send(()).await.unwrap();
        task.await.unwrap();
        assert!(
            client
                .call(
                    "after-shutdown",
                    Request::GetStatus,
                    std::time::Duration::from_secs(1)
                )
                .await
                .is_err(),
            "a shutdown server must not leave live sessions processing requests"
        );
    }

    #[tokio::test]
    async fn p1_shutdown_closes_a_session_with_an_inflight_handler() {
        let path = format!(
            r"\\.\pipe\aicore-p1-inflight-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let handler = Arc::new(super::deadline_tests::BlockedHandler {
            inline_fixture: true,
            ..Default::default()
        });
        let (shutdown, rx) = mpsc::channel(1);
        let server = IpcServer::bind(&path, handler.clone())
            .unwrap()
            .with_shutdown(rx);
        let server_task = tokio::spawn(server.run());
        let mut client = IpcClient::connect(&path).await.unwrap();
        let client_task = tokio::spawn(async move {
            client
                .call(
                    "blocked",
                    Request::GetStatus,
                    std::time::Duration::from_secs(2),
                )
                .await
        });
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(1);
        while !handler.started.load(Ordering::SeqCst) && tokio::time::Instant::now() < deadline {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
        assert!(handler.started.load(Ordering::SeqCst));
        shutdown.send(()).await.unwrap();
        tokio::time::timeout(std::time::Duration::from_millis(100), server_task)
            .await
            .unwrap()
            .unwrap();
        let result = tokio::time::timeout(std::time::Duration::from_millis(100), client_task).await;
        handler.release.store(true, Ordering::SeqCst);
        assert!(result.unwrap().unwrap().is_err());
        while !handler.finished.load(Ordering::SeqCst) && tokio::time::Instant::now() < deadline {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
        assert!(handler.finished.load(Ordering::SeqCst));
    }
}

/// IPC Server。
pub struct IpcServer {
    listener: IpcListener,
    handler: Arc<dyn IpcHandler>,
    shutdown_rx: mpsc::Receiver<()>,
    max_connections: Arc<Semaphore>,
    handler_slots: Arc<Semaphore>,
}

impl IpcServer {
    /// Creates a server, binds to the specified path.
    pub fn bind(path: impl Into<PathBuf>, handler: Arc<dyn IpcHandler>) -> std::io::Result<Self> {
        let listener = server_listen(path.into())?;
        let (_tx, rx) = mpsc::channel(1);
        Ok(Self {
            listener,
            handler,
            shutdown_rx: rx,
            max_connections: Arc::new(Semaphore::new(32)),
            handler_slots: Arc::new(Semaphore::new(32)),
        })
    }

    pub fn path(&self) -> &std::path::Path {
        self.listener.path()
    }

    /// Sets the shutdown channel.
    pub fn with_shutdown(mut self, rx: mpsc::Receiver<()>) -> Self {
        self.shutdown_rx = rx;
        self
    }

    /// Starts the event cycle.
    pub async fn run(mut self) {
        tracing::info!(
            component = "ipc_server",
            event = "started",
            path = %self.listener.path().display(),
            handler = self.handler.name(),
            "ipc server started"
        );

        let mut connections = tokio::task::JoinSet::new();
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

                _ = connections.join_next(), if !connections.is_empty() => {}

                result = self.listener.accept() => {
                    match result {
                        Ok(stream) => {
                            let handler = self.handler.clone();
                            let permits = self.max_connections.clone();
                            let handler_slots = self.handler_slots.clone();
                            let permit = match permits.try_acquire_owned() {
                                Ok(permit) => permit,
                                Err(_) => {
                                    tracing::warn!(component = "ipc_server", event = "connection_rejected", "connection limit reached");
                                    continue;
                                },
                            };
                            connections.spawn(async move {
                                if let Err(e) = handle_connection(stream, handler, permit, handler_slots).await {
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
        connections.abort_all();
        while connections.join_next().await.is_some() {}
    }
}

async fn handle_connection(
    mut stream: IpcStream,
    handler: Arc<dyn IpcHandler>,
    _permit: OwnedSemaphorePermit,
    handler_slots: Arc<Semaphore>,
) -> std::io::Result<()> {
    let identity = stream.peer_identity();
    let mut seen = HashSet::<String>::new();
    let mut acknowledgements = false;
    let mut selected_version = IPC_PROTOCOL_VERSION;
    loop {
        let deadline = tokio::time::Instant::now() + CONNECTION_IDLE_TIMEOUT;
        let line = match tokio::time::timeout_at(deadline, stream.read_message()).await {
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
                    let response = dispatch_connected(
                        &mut stream,
                        handler.clone(),
                        envelope,
                        identity,
                        handler_slots.clone(),
                        deadline.min(tokio::time::Instant::now() + HANDLER_TIMEOUT),
                    )
                    .await?;
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
                        let mut response = dispatch_connected(
                            &mut stream,
                            handler.clone(),
                            canonical,
                            identity,
                            handler_slots.clone(),
                            deadline.min(tokio::time::Instant::now() + HANDLER_TIMEOUT),
                        )
                        .await?;
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
            write_response(&mut stream, &ack, deadline).await?;
        }
        write_response(&mut stream, &response, deadline).await?;
        if seen.len() >= 4096 {
            break;
        }
    }
    Ok(())
}

async fn dispatch_connected(
    stream: &mut IpcStream,
    handler: Arc<dyn IpcHandler>,
    envelope: Envelope,
    identity: ClientIdentity,
    slots: Arc<Semaphore>,
    deadline: tokio::time::Instant,
) -> std::io::Result<ResponseEnvelope> {
    if handler.process_spec().is_none() {
        return Ok(dispatch_blocking(handler, envelope, identity, slots, deadline).await);
    }
    tokio::select! {
        response = dispatch_blocking(handler, envelope, identity, slots, deadline) => Ok(response),
        incoming = stream.read_message() => {
            match incoming? {
                None => Err(std::io::Error::new(std::io::ErrorKind::ConnectionAborted, "client disconnected; handler process reaped")),
                Some(_) => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "pipelined requests are not supported by process handlers")),
            }
        }
    }
}

pub(crate) async fn dispatch_blocking(
    handler: Arc<dyn IpcHandler>,
    envelope: Envelope,
    identity: ClientIdentity,
    slots: Arc<Semaphore>,
    deadline: tokio::time::Instant,
) -> ResponseEnvelope {
    let request_id = envelope.request_id.clone();
    let permit = match slots.try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => {
            return ResponseEnvelope::err(
                request_id,
                IpcError::new("ERR_RESOURCE_UNAVAILABLE", "IPC handler capacity reached"),
            )
        },
    };
    if let Some(spec) = handler.process_spec() {
        let state = match handler.prepare_process_state(&envelope, identity, deadline.into_std()) {
            Ok(state) => state,
            Err(error) => return ResponseEnvelope::err(request_id, error),
        };
        return crate::process_handler::dispatch(
            spec,
            crate::process_handler::ProcessHandlerRequest {
                envelope,
                identity,
                state,
            },
            handler,
            permit,
            deadline,
        )
        .await;
    }
    if tokio::time::Instant::now() >= deadline {
        return ResponseEnvelope::err(
            request_id,
            IpcError::new("ERR_TIMEOUT", "deadline expired before execution"),
        );
    }
    #[cfg(not(test))]
    {
        drop(permit);
        ResponseEnvelope::err(
            request_id,
            IpcError::not_supported(
                "synchronous IPC handlers have no termination guarantee; managed process required",
            ),
        )
    }
    #[cfg(test)]
    if !handler.inline_test_fixture() {
        return ResponseEnvelope::err(
            request_id,
            IpcError::not_supported("managed process required"),
        );
    }
    #[cfg(test)]
    {
        dispatch_inline_fixture(handler, envelope, identity, permit, deadline).await
    }
}

#[cfg(test)]
async fn dispatch_inline_fixture(
    handler: Arc<dyn IpcHandler>,
    envelope: Envelope,
    identity: ClientIdentity,
    permit: OwnedSemaphorePermit,
    deadline: tokio::time::Instant,
) -> ResponseEnvelope {
    let request_id = envelope.request_id.clone();
    let (finished, result) = tokio::sync::oneshot::channel();
    let worker_id = request_id.clone();
    let worker = std::thread::Builder::new()
        .name("aicore-ipc-handler".into())
        .spawn(move || {
            // Keep capacity until synchronous work actually stops, even when the
            // connection times out or shuts down. Never grow an unbounded pool.
            let _permit = permit;
            if finished.is_closed() || std::time::Instant::now() >= deadline.into_std() {
                return;
            }
            let response = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                process_envelope(&handler, envelope, identity)
            }))
            .unwrap_or_else(|_| {
                ResponseEnvelope::err(worker_id, IpcError::internal("IPC handler panicked"))
            });
            let _ = finished.send(response);
        });
    if let Err(error) = worker {
        return ResponseEnvelope::err(request_id, IpcError::internal(error.to_string()));
    }
    match tokio::time::timeout_at(deadline, result).await {
        Ok(Ok(response)) => response,
        Ok(Err(error)) => ResponseEnvelope::err(request_id, IpcError::internal(error.to_string())),
        Err(_) => ResponseEnvelope::err(
            request_id,
            IpcError::new(
                "ERR_TIMEOUT",
                "IPC handler deadline exceeded; in-flight work may still finish",
            ),
        ),
    }
}

async fn write_response(
    stream: &mut IpcStream,
    response: &ResponseEnvelope,
    deadline: tokio::time::Instant,
) -> std::io::Result<()> {
    let json = serde_json::to_string(response).unwrap_or_else(|_| {
            r#"{"version":"1.0","request_id":"unknown","success":false,"error":{"code":"ERR_INTERNAL","message":"serialize failed"}}"#.to_string()
        });
    tokio::time::timeout_at(deadline, stream.write_message(&json))
        .await
        .map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::TimedOut, "IPC response write timed out")
        })?
}

/// Direct protocol processing for worker/local callers only. Network dispatch
/// deliberately never calls a synchronous handler outside a managed process.
pub fn process_envelope(
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
        fn inline_test_fixture(&self) -> bool {
            true
        }
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

#[cfg(test)]
mod deadline_tests {
    use super::*;
    #[derive(Default)]
    pub(super) struct BlockedHandler {
        pub(super) inline_fixture: bool,
        pub(super) started: std::sync::atomic::AtomicBool,
        pub(super) release: std::sync::atomic::AtomicBool,
        pub(super) finished: std::sync::atomic::AtomicBool,
    }
    impl IpcHandler for BlockedHandler {
        fn inline_test_fixture(&self) -> bool {
            self.inline_fixture
        }
        fn handle(&self, _: Request) -> Result<ResponseData, IpcError> {
            use std::sync::atomic::Ordering;
            self.started.store(true, Ordering::SeqCst);
            let start = std::time::Instant::now();
            while !self.release.load(Ordering::SeqCst)
                && start.elapsed() < std::time::Duration::from_secs(1)
            {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            self.finished.store(true, Ordering::SeqCst);
            Ok(ResponseData::Ack)
        }
        fn name(&self) -> &'static str {
            "blocked"
        }
    }

    #[tokio::test]
    async fn p1_expired_handler_deadline_never_starts_work() {
        let handler = Arc::new(BlockedHandler::default());
        let response = dispatch_blocking(
            handler.clone(),
            Envelope::new("expired", Request::GetStatus),
            ClientIdentity::Internal,
            Arc::new(Semaphore::new(1)),
            tokio::time::Instant::now() - std::time::Duration::from_millis(1),
        )
        .await;
        assert_eq!(response.error.unwrap().code, "ERR_TIMEOUT");
        assert!(!handler.started.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[tokio::test]
    async fn p1_timed_out_handler_keeps_capacity_until_it_actually_finishes() {
        use std::sync::atomic::Ordering;
        let handler = Arc::new(BlockedHandler {
            inline_fixture: true,
            ..Default::default()
        });
        let slots = Arc::new(Semaphore::new(1));
        let response = dispatch_blocking(
            handler.clone(),
            Envelope::new("blocked", Request::GetStatus),
            ClientIdentity::Internal,
            slots.clone(),
            tokio::time::Instant::now() + std::time::Duration::from_millis(30),
        )
        .await;
        assert_eq!(response.error.unwrap().code, "ERR_TIMEOUT");
        assert!(handler.started.load(Ordering::SeqCst));
        let busy = dispatch_blocking(
            handler.clone(),
            Envelope::new("busy", Request::GetStatus),
            ClientIdentity::Internal,
            slots.clone(),
            tokio::time::Instant::now() + std::time::Duration::from_secs(1),
        )
        .await;
        assert_eq!(busy.error.unwrap().code, "ERR_RESOURCE_UNAVAILABLE");
        handler.release.store(true, Ordering::SeqCst);
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(1);
        while slots.available_permits() == 0 && tokio::time::Instant::now() < deadline {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
        assert_eq!(slots.available_permits(), 1);
    }

    #[tokio::test]
    async fn p1_handler_deadline_must_not_leave_background_execution() {
        use std::sync::atomic::Ordering;
        let handler = Arc::new(BlockedHandler::default());
        let response = dispatch_blocking(
            handler.clone(),
            Envelope::new("termination", Request::GetStatus),
            ClientIdentity::Internal,
            Arc::new(Semaphore::new(1)),
            tokio::time::Instant::now() + std::time::Duration::from_millis(30),
        )
        .await;
        assert_eq!(response.error.unwrap().code, "ERR_NOT_SUPPORTED");
        assert!(
            !handler.started.load(Ordering::SeqCst),
            "unsafe synchronous Handler started despite no termination mechanism"
        );
        assert!(
            !handler.finished.load(Ordering::SeqCst),
            "refusal must not be represented as completed execution"
        );
    }
    struct SlowHandler;
    impl IpcHandler for SlowHandler {
        fn inline_test_fixture(&self) -> bool {
            true
        }
        fn handle(&self, _: Request) -> Result<ResponseData, IpcError> {
            std::thread::sleep(std::time::Duration::from_secs(6));
            Ok(ResponseData::Ack)
        }
        fn name(&self) -> &'static str {
            "slow"
        }
    }
    #[tokio::test]
    async fn p1_handler_response_has_a_deadline() {
        let start = std::time::Instant::now();
        let response = dispatch_blocking(
            Arc::new(SlowHandler),
            Envelope::new("slow", Request::GetStatus),
            ClientIdentity::Internal,
            Arc::new(Semaphore::new(1)),
            tokio::time::Instant::now() + HANDLER_TIMEOUT,
        )
        .await;
        assert!(
            !response.success,
            "handler over deadline must not report success"
        );
        assert_eq!(response.error.unwrap().code, "ERR_TIMEOUT");
        assert!(start.elapsed() < std::time::Duration::from_millis(5500));
        // Finite test provider: let its detached work finish before test exit.
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    }
}
