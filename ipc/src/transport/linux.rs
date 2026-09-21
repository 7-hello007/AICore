use std::io;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

use super::IpcTransport;

/// Linux Unix Domain Socket 传输。
pub struct IpcStream {
    reader: BufReader<tokio::io::ReadHalf<UnixStream>>,
    writer: tokio::io::WriteHalf<UnixStream>,
}

impl IpcStream {
    fn from_stream(stream: UnixStream) -> Self {
        let (read, write) = tokio::io::split(stream);
        Self {
            reader: BufReader::new(read),
            writer: write,
        }
    }
}

#[async_trait]
impl IpcTransport for IpcStream {
    async fn read_message(&mut self) -> io::Result<Option<String>> {
        let mut line = String::new();
        let n = self.reader.read_line(&mut line).await?;
        if n == 0 {
            return Ok(None);
        }
        Ok(Some(line.trim_end_matches(['\r', '\n']).to_string()))
    }

    async fn write_message(&mut self, msg: &str) -> io::Result<()> {
        self.writer.write_all(msg.as_bytes()).await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;
        Ok(())
    }
}

/// Unix Domain Socket 监听器。
pub struct IpcListener {
    inner: UnixListener,
    path: PathBuf,
}

impl IpcListener {
    /// 接受一个连接。
    pub async fn accept(&self) -> io::Result<IpcStream> {
        let (stream, _) = self.inner.accept().await?;
        Ok(IpcStream::from_stream(stream))
    }

    /// 返回监听路径。
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for IpcListener {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// 监听 Unix Domain Socket。
///
/// 若 `path` 已存在且是 socket，会先删除，避免 "address already in use"。
pub fn server_listen(path: impl AsRef<Path>) -> io::Result<IpcListener> {
    let path = path.as_ref().to_path_buf();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if path.exists() {
        let _ = std::fs::remove_file(&path);
    }
    let inner = UnixListener::bind(&path)?;
    Ok(IpcListener { inner, path })
}

/// 连接 Unix Domain Socket。
pub async fn client_connect(path: impl AsRef<Path>) -> io::Result<IpcStream> {
    let stream = UnixStream::connect(path.as_ref()).await?;
    Ok(IpcStream::from_stream(stream))
}
