use std::io;

use async_trait::async_trait;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::{client_connect, server_listen, IpcListener, IpcStream};
#[cfg(target_os = "windows")]
pub use windows::{client_connect, server_listen, IpcListener, IpcStream};

/// 传输层抽象。
#[async_trait]
pub trait IpcTransport: Send + Sync {
    async fn read_message(&mut self) -> io::Result<Option<String>>;
    async fn write_message(&mut self, msg: &str) -> io::Result<()>;
}
