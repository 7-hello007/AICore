use std::io;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

use super::{read_bounded_message, IpcTransport};
use crate::ClientIdentity;

/// Linux Unix Domain Socket 传输。
pub struct IpcStream {
    reader: BufReader<tokio::io::ReadHalf<UnixStream>>,
    writer: tokio::io::WriteHalf<UnixStream>,
    identity: ClientIdentity,
}

impl IpcStream {
    fn from_stream(stream: UnixStream, identity: ClientIdentity) -> Self {
        let (read, write) = tokio::io::split(stream);
        Self {
            reader: BufReader::new(read),
            writer: write,
            identity,
        }
    }

    pub fn peer_identity(&self) -> ClientIdentity {
        self.identity
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
    owner_uid: u32,
    socket_dev: u64,
    socket_ino: u64,
}

impl IpcListener {
    /// 接受一个连接。
    pub async fn accept(&self) -> io::Result<IpcStream> {
        let (stream, _) = self.inner.accept().await?;
        let credentials = stream.peer_cred()?;
        if credentials.uid() != self.owner_uid {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "IPC peer identity unavailable or not the socket owner",
            ));
        }
        let pid = credentials.pid().ok_or_else(|| {
            io::Error::new(io::ErrorKind::PermissionDenied, "IPC peer PID unavailable")
        })?;
        let start_time_ticks = read_process_start_time(pid)?;
        let (executable_dev, executable_ino) = read_process_executable_identity(pid)?;
        Ok(IpcStream::from_stream(
            stream,
            ClientIdentity::Linux {
                uid: credentials.uid(),
                pid,
                start_time_ticks,
                executable_dev,
                executable_ino,
            },
        ))
    }

    /// 返回监听路径。
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for IpcListener {
    fn drop(&mut self) {
        use std::os::unix::fs::MetadataExt;
        if let Ok(metadata) = std::fs::symlink_metadata(&self.path) {
            if metadata.dev() == self.socket_dev && metadata.ino() == self.socket_ino {
                let _ = std::fs::remove_file(&self.path);
            }
        }
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
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    use std::os::unix::fs::MetadataExt;
    let metadata = std::fs::symlink_metadata(&path)?;
    Ok(IpcListener {
        inner,
        path,
        owner_uid: metadata.uid(),
        socket_dev: metadata.dev(),
        socket_ino: metadata.ino(),
    })
}

/// 连接 Unix Domain Socket。
pub async fn client_connect(path: impl AsRef<Path>) -> io::Result<IpcStream> {
    let stream = UnixStream::connect(path.as_ref()).await?;
    Ok(IpcStream::from_stream(
        stream,
        ClientIdentity::Unauthenticated,
    ))
}

pub(crate) fn read_process_executable_identity(pid: i32) -> io::Result<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    let metadata = std::fs::metadata(format!("/proc/{pid}/exe"))?;
    Ok((metadata.dev(), metadata.ino()))
}

pub(crate) fn read_process_start_time(pid: i32) -> io::Result<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"))?;
    parse_process_start_time(&stat).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            "IPC peer process start time unavailable",
        )
    })
}

fn parse_process_start_time(stat: &str) -> Option<u64> {
    // The comm field is parenthesized and may contain spaces or ')'. The
    // suffix starts at field 3 (state); starttime is field 22, index 19.
    stat.rsplit_once(") ")?
        .1
        .split_whitespace()
        .nth(19)?
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    #[test]
    fn process_start_time_parser_handles_parentheses_in_comm() {
        let mut fields = vec!["S".to_string(); 20];
        fields[19] = "12345".into();
        let stat = format!("9 (worker ) unusual) {}", fields.join(" "));
        assert_eq!(super::parse_process_start_time(&stat), Some(12345));
    }
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
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&socket).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(super::server_listen(&socket).is_err());
        assert!(socket.exists());
        drop(listener);

        let replaced = root.join("replaced.sock");
        let listener = super::server_listen(&replaced).unwrap();
        std::fs::remove_file(&replaced).unwrap();
        std::fs::write(&replaced, "replacement").unwrap();
        drop(listener);
        assert_eq!(std::fs::read_to_string(&replaced).unwrap(), "replacement");
        std::fs::remove_file(&replaced).unwrap();
    }
}
