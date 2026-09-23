use std::io;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

use super::{read_bounded_message, IpcTransport};

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
        read_bounded_message(&mut self.reader).await
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
/// 仅清理已失效的 socket；保留普通文件、符号链接和正在监听的 socket。
pub fn server_listen(path: impl AsRef<Path>) -> io::Result<IpcListener> {
    let path = path.as_ref().to_path_buf();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if let Ok(metadata) = std::fs::symlink_metadata(&path) {
        use std::os::unix::fs::FileTypeExt;
        if !metadata.file_type().is_socket() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "IPC path is not a socket",
            ));
        }
        match std::os::unix::net::UnixStream::connect(&path) {
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::AddrInUse,
                    "IPC socket is active",
                ))
            },
            Err(error) if error.kind() == io::ErrorKind::ConnectionRefused => {
                std::fs::remove_file(&path)?;
            },
            Err(error) => return Err(error),
        }
    }
    let inner = UnixListener::bind(&path)?;
    Ok(IpcListener { inner, path })
}

/// 连接 Unix Domain Socket。
pub async fn client_connect(path: impl AsRef<Path>) -> io::Result<IpcStream> {
    let stream = UnixStream::connect(path.as_ref()).await?;
    Ok(IpcStream::from_stream(stream))
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn listener_preserves_existing_files_and_active_sockets() {
        let root = std::env::temp_dir().join(format!("aicore-ipc-preserve-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("keep.txt");
        std::fs::write(&path, "preserve").unwrap();
        assert!(super::server_listen(&path).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "preserve");
        let socket = root.join("active.sock");
        let listener = super::server_listen(&socket).unwrap();
        assert!(super::server_listen(&socket).is_err());
        assert!(socket.exists());
        drop(listener);
    }
}
