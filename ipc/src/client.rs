use std::path::PathBuf;
use std::time::Duration;

use aicore_contracts::ipc::protocol::{
    Envelope, IpcError, Request, ResponseData, ResponseEnvelope, IPC_PROTOCOL_VERSION,
};

use crate::transport::{client_connect, IpcStream, IpcTransport};

/// A simple IPC client. A request for a response once.
pub struct IpcClient {
    stream: Option<IpcStream>,
    ack_enabled: bool,
    version: aicore_contracts::schema::SchemaVersion,
    failed: bool,
}

impl IpcClient {
    /// Connect to IPC server.
    pub async fn connect(path: impl Into<PathBuf>) -> std::io::Result<Self> {
        Self::connect_with_versions(path, vec![IPC_PROTOCOL_VERSION]).await
    }

    pub async fn connect_with_versions(
        path: impl Into<PathBuf>,
        versions: Vec<aicore_contracts::schema::SchemaVersion>,
    ) -> std::io::Result<Self> {
        if versions.is_empty()
            || versions
                .iter()
                .any(|v| !aicore_contracts::ipc::protocol::IPC_SUPPORTED_VERSIONS.contains(v))
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "unsupported client wire version",
            ));
        }
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let mut stream = tokio::time::timeout_at(deadline, client_connect(path.into()))
            .await
            .map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::TimedOut, "IPC connect timed out")
            })??;
        let handshake_id = format!("aicore-handshake-{}", std::process::id());
        let mut handshake = Envelope::new(
            &handshake_id,
            Request::Handshake {
                capabilities: vec!["request_ack".to_string()],
                supported_versions: versions.clone(),
            },
        );
        handshake.version = versions[0];
        let json = serde_json::to_string(&handshake).map_err(std::io::Error::other)?;
        tokio::time::timeout_at(deadline, stream.write_message(&json))
            .await
            .map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "IPC handshake write timed out",
                )
            })??;
        let response = read_response_until(&mut stream, deadline).await?;
        if response.request_id != handshake_id || !versions.contains(&response.version) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "handshake request_id mismatch",
            ));
        }
        let version = response.version;
        let ack_enabled = match (response.success, response.data, response.error) {
            (
                true,
                Some(ResponseData::HandshakeAck {
                    capabilities,
                    selected_version,
                }),
                _,
            ) if selected_version == version => capabilities
                .iter()
                .any(|capability| capability == "request_ack"),
            (false, _, Some(error)) if error.code == "ERR_NOT_SUPPORTED" => false,
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "IPC handshake rejected or malformed",
                ));
            },
        };
        Ok(Self {
            stream: Some(stream),
            ack_enabled,
            version,
            failed: false,
        })
    }

    /// Sends a request and waits for a response.
    pub async fn call(
        &mut self,
        request_id: &str,
        request: Request,
        timeout: Duration,
    ) -> std::io::Result<ResponseEnvelope> {
        if self.failed {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "IPC session failed; reconnect before reuse",
            ));
        }
        let deadline = tokio::time::Instant::now()
            .checked_add(timeout)
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid IPC timeout")
            })?;
        if request_id.trim().is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "request_id must not be empty",
            ));
        }
        if matches!(request, Request::Handshake { .. }) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "handshake is performed by connect",
            ));
        }
        let mut envelope = Envelope::new(request_id, request);
        envelope.version = self.version;
        let json = serde_json::to_string(&envelope)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        if json.len() >= crate::transport::MAX_MESSAGE_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "IPC request too large",
            ));
        }
        // Cancellation or partial write/read invalidates framing. A timed-out
        // response must never be consumed as the next request's response.
        self.failed = true;
        // The future owns the transport while a request is active. Every
        // error and future cancellation closes it, notifying a managed server
        // to terminate its Handler even if this client object stays alive.
        let mut stream = self.stream.take().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::BrokenPipe, "IPC transport closed")
        })?;
        tokio::time::timeout_at(deadline, stream.write_message(&json))
            .await
            .map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::TimedOut, "IPC request write timed out")
            })??;
        if self.ack_enabled {
            let acknowledgement = read_response_until(&mut stream, deadline).await?;
            if acknowledgement.request_id != request_id
                || acknowledgement.version != self.version
                || !acknowledgement.success
                || !matches!(acknowledgement.data, Some(ResponseData::Ack))
            {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "missing or mismatched IPC acknowledgement",
                ));
            }
        }
        let response = read_response_until(&mut stream, deadline).await?;
        if response.request_id != request_id {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "IPC response request_id mismatch",
            ));
        }
        if response.version != self.version {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "IPC response version mismatch",
            ));
        }
        self.failed = false;
        self.stream = Some(stream);
        Ok(response)
    }
    pub fn negotiated_version(&self) -> aicore_contracts::schema::SchemaVersion {
        self.version
    }
}

async fn read_response_until(
    stream: &mut IpcStream,
    deadline: tokio::time::Instant,
) -> std::io::Result<ResponseEnvelope> {
    let line = tokio::time::timeout_at(deadline, stream.read_message())
        .await
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "ipc request timed out"))??
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "server closed connection",
            )
        })?;
    serde_json::from_str(&line)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}

/// Extract data or error from `ResponseEnvelope`.
pub fn unwrap_response(resp: ResponseEnvelope) -> Result<ResponseData, IpcError> {
    if resp.success {
        resp.data
            .ok_or_else(|| IpcError::internal("missing data in success response"))
    } else {
        Err(resp
            .error
            .unwrap_or_else(|| IpcError::internal("missing error in failure response")))
    }
}

#[cfg(all(test, target_os = "windows"))]
mod deadline_tests {
    use super::*;
    use crate::transport::server_listen;
    #[tokio::test]
    async fn p1_blocked_request_write_is_inside_call_deadline() {
        let path = format!(
            r"\\.\pipe\aicore-p1-write-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let listener = server_listen(&path).unwrap();
        let (release, resume) = tokio::sync::oneshot::channel::<()>();
        let peer = tokio::spawn(async move {
            let mut stream = listener.accept().await.unwrap();
            let handshake: Envelope =
                serde_json::from_str(&stream.read_message().await.unwrap().unwrap()).unwrap();
            let response = ResponseEnvelope::ok(
                handshake.request_id,
                ResponseData::HandshakeAck {
                    capabilities: vec![],
                    selected_version: IPC_PROTOCOL_VERSION,
                },
            );
            stream
                .write_message(&serde_json::to_string(&response).unwrap())
                .await
                .unwrap();
            let _ = resume.await;
        });
        let mut client = IpcClient::connect(path).await.unwrap();
        let request = Request::Register {
            application_id: aicore_contracts::application::id::ApplicationId::new("p1"),
            name: "x".repeat(512 * 1024),
            version: "1".into(),
            capabilities: vec![],
        };
        let result = tokio::time::timeout(
            Duration::from_millis(250),
            client.call("blocked", request, Duration::from_millis(30)),
        )
        .await;
        let _ = release.send(());
        peer.await.unwrap();
        assert!(
            matches!(result, Ok(Err(ref error)) if error.kind() == std::io::ErrorKind::TimedOut),
            "write must obey the caller's deadline"
        );
        assert!(client
            .call("reuse", Request::GetStatus, Duration::from_millis(30))
            .await
            .is_err());
    }
}
