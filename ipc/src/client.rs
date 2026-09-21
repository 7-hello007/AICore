use std::path::PathBuf;
use std::time::Duration;

use aicore_contracts::ipc::protocol::{
    Envelope, IpcError, Request, ResponseData, ResponseEnvelope,
};

use crate::transport::{client_connect, IpcStream, IpcTransport};

/// 简单的 IPC 客户端。一次请求一次响应。
pub struct IpcClient {
    stream: IpcStream,
}

impl IpcClient {
    /// 连接到 IPC server。
    pub async fn connect(path: impl Into<PathBuf>) -> std::io::Result<Self> {
        let stream = client_connect(path.into()).await?;
        Ok(Self { stream })
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
        let envelope = Envelope::new(request_id, request);
        let json = serde_json::to_string(&envelope)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        self.stream.write_message(&json).await?;

        let read_fut = self.stream.read_message();
        match tokio::time::timeout(timeout, read_fut).await {
            Ok(Ok(Some(line))) => {
                let resp: ResponseEnvelope = serde_json::from_str(&line)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                Ok(resp)
            },
            Ok(Ok(None)) => Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "server closed connection",
            )),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "ipc request timed out",
            )),
        }
    }
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
