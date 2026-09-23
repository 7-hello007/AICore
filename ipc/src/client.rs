use std::path::PathBuf;
use std::time::Duration;

use aicore_contracts::ipc::protocol::{
    Envelope, IpcError, Request, ResponseData, ResponseEnvelope,
};

use crate::transport::{client_connect, IpcStream, IpcTransport};

/// 简单的 IPC 客户端。一次请求一次响应。
pub struct IpcClient {
    stream: IpcStream,
    ack_enabled: bool,
}

impl IpcClient {
    /// 连接到 IPC server。
    pub async fn connect(path: impl Into<PathBuf>) -> std::io::Result<Self> {
        let mut stream = client_connect(path.into()).await?;
        let handshake_id = format!("aicore-handshake-{}", std::process::id());
        let handshake = Envelope::new(
            &handshake_id,
            Request::Handshake {
                capabilities: vec!["request_ack".to_string()],
            },
        );
        let json = serde_json::to_string(&handshake).map_err(std::io::Error::other)?;
        stream.write_message(&json).await?;
        let response = read_response(&mut stream, Duration::from_secs(5)).await?;
        if response.request_id != handshake_id {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "handshake request_id mismatch",
            ));
        }
        let ack_enabled = match (response.success, response.data, response.error) {
            (true, Some(ResponseData::HandshakeAck { capabilities }), _) => capabilities
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
            stream,
            ack_enabled,
        })
    }

    /// 发送请求并等待响应，超时由调用方提供。
    pub async fn call(
        &mut self,
        request_id: &str,
        request: Request,
        timeout: Duration,
    ) -> std::io::Result<ResponseEnvelope> {
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
        let envelope = Envelope::new(request_id, request);
        let json = serde_json::to_string(&envelope)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        self.stream.write_message(&json).await?;

        let deadline = tokio::time::Instant::now() + timeout;
        if self.ack_enabled {
            let acknowledgement = read_response_until(&mut self.stream, deadline).await?;
            if acknowledgement.request_id != request_id
                || !acknowledgement.success
                || !matches!(acknowledgement.data, Some(ResponseData::Ack))
            {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "missing or mismatched IPC acknowledgement",
                ));
            }
        }
        let response = read_response_until(&mut self.stream, deadline).await?;
        if response.request_id != request_id {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "IPC response request_id mismatch",
            ));
        }
        Ok(response)
    }
}

async fn read_response(
    stream: &mut IpcStream,
    timeout: Duration,
) -> std::io::Result<ResponseEnvelope> {
    read_response_until(stream, tokio::time::Instant::now() + timeout).await
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

/// 从 `ResponseEnvelope` 提取数据或错误。
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
