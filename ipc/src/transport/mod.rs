use std::io;

use async_trait::async_trait;

pub const MAX_MESSAGE_BYTES: usize = 1024 * 1024;

async fn read_bounded_message<R: tokio::io::AsyncBufRead + Unpin>(
    reader: &mut R,
) -> io::Result<Option<String>> {
    use tokio::io::{AsyncBufReadExt, AsyncReadExt};
    let mut line = String::new();
    let n = reader
        .take((MAX_MESSAGE_BYTES + 1) as u64)
        .read_line(&mut line)
        .await?;
    if n == 0 {
        return Ok(None);
    }
    if n > MAX_MESSAGE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "IPC message exceeds MAX_MESSAGE_BYTES",
        ));
    }
    Ok(Some(line.trim_end_matches(['\r', '\n']).to_string()))
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn oversized_message_without_newline_is_bounded() {
        use tokio::io::{AsyncWriteExt, BufReader};
        let (mut writer, reader) = tokio::io::duplex(super::MAX_MESSAGE_BYTES + 1);
        writer
            .write_all(&vec![b'x'; super::MAX_MESSAGE_BYTES + 1])
            .await
            .unwrap();
        // Keep writer open: the reader must reject before EOF or newline.
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            super::read_bounded_message(&mut BufReader::new(reader)),
        )
        .await
        .unwrap();
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidData);
    }
}

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::{client_connect, server_listen, IpcListener, IpcStream};
#[cfg(target_os = "linux")]
pub(crate) use linux::{read_process_executable_identity, read_process_start_time};
#[cfg(target_os = "windows")]
pub use windows::{client_connect, server_listen, IpcListener, IpcStream};
#[cfg(target_os = "windows")]
pub(crate) use windows::{process_creation_time, process_executable_path, process_identity_stale};

/// 传输层抽象。
#[async_trait]
pub trait IpcTransport: Send + Sync {
    async fn read_message(&mut self) -> io::Result<Option<String>>;
    async fn write_message(&mut self, msg: &str) -> io::Result<()>;
}
#[cfg(target_os = "windows")]
pub(crate) use windows::process_user_sid;
