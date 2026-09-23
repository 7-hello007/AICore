use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use aicore_contracts::ipc::protocol::{
    Envelope, IpcError, Request, ResponseData, ResponseEnvelope, IPC_PROTOCOL_VERSION,
};
use tokio::sync::{mpsc, Semaphore};

use crate::transport::{server_listen, IpcListener, IpcStream, IpcTransport};

/// 请求处理器。daemon 实现此 trait，把 IPC 请求映射到内部服务。
pub trait IpcHandler: Send + Sync + 'static {
    /// 处理一条请求并返回响应数据；错误通过 `Err` 返回。
    fn handle(&self, request: Request) -> Result<ResponseData, IpcError>;

    /// Handler 名称，用于日志。
    fn name(&self) -> &'static str;
}

/// IPC Server。
pub struct IpcServer {
    listener: IpcListener,
    handler: Arc<dyn IpcHandler>,
    shutdown_rx: mpsc::Receiver<()>,
    max_in_flight: Arc<Semaphore>,
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
            max_in_flight: Arc::new(Semaphore::new(32)),
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
                            let permits = self.max_in_flight.clone();
                            tokio::spawn(async move {
                                if let Err(e) = handle_connection(stream, handler, permits).await {
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
    permits: Arc<Semaphore>,
) -> std::io::Result<()> {
    let mut seen = HashSet::<String>::new();
    loop {
        let line = match stream.read_message().await? {
            Some(l) => l,
            None => break,
        };

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
                } else {
                    match permits.clone().try_acquire_owned() {
                        Ok(_permit) => process_envelope(&handler, envelope),
                        Err(_) => ResponseEnvelope::err(
                            envelope.request_id,
                            IpcError::new("ERR_BACKPRESSURE", "too many in-flight requests"),
                        ),
                    }
                }
            },
            Err(e) => ResponseEnvelope::err(
                "unknown",
                IpcError::invalid_request(format!("cannot parse envelope: {}", e)),
            ),
        };

        let json = serde_json::to_string(&response).unwrap_or_else(|_| {
            r#"{"version":"1.0","request_id":"unknown","success":false,"error":{"code":"ERR_INTERNAL","message":"serialize failed"}}"#.to_string()
        });
        stream.write_message(&json).await?;
        if seen.len() > 4096 {
            seen.clear();
        }
    }
    Ok(())
}

fn process_envelope(handler: &Arc<dyn IpcHandler>, envelope: Envelope) -> ResponseEnvelope {
    if envelope.version != IPC_PROTOCOL_VERSION {
        return ResponseEnvelope::err(
            envelope.request_id,
            IpcError::protocol_mismatch(envelope.version, IPC_PROTOCOL_VERSION),
        );
    }

    let request_id = envelope.request_id.clone();
    match handler.handle(envelope.payload) {
        Ok(data) => ResponseEnvelope::ok(request_id, data),
        Err(e) => ResponseEnvelope::err(request_id, e),
    }
}
