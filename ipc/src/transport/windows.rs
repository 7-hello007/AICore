use std::io;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::{
    ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
};

use super::{read_bounded_message, IpcTransport};

type ServerRead = tokio::io::ReadHalf<NamedPipeServer>;
type ServerWrite = tokio::io::WriteHalf<NamedPipeServer>;
type ClientRead = tokio::io::ReadHalf<NamedPipeClient>;
type ClientWrite = tokio::io::WriteHalf<NamedPipeClient>;

pub enum IpcStream {
    Server {
        reader: BufReader<ServerRead>,
        writer: ServerWrite,
    },
    Client {
        reader: BufReader<ClientRead>,
        writer: ClientWrite,
    },
}

impl IpcStream {
    fn from_server(stream: NamedPipeServer) -> Self {
        let (read, write) = tokio::io::split(stream);
        Self::Server {
            reader: BufReader::new(read),
            writer: write,
        }
    }
    fn from_client(stream: NamedPipeClient) -> Self {
        let (read, write) = tokio::io::split(stream);
        Self::Client {
            reader: BufReader::new(read),
            writer: write,
        }
    }
}

#[async_trait]
impl IpcTransport for IpcStream {
    async fn read_message(&mut self) -> io::Result<Option<String>> {
        match self {
            Self::Server { reader, .. } => read_bounded_message(reader).await,
            Self::Client { reader, .. } => read_bounded_message(reader).await,
        }
    }

    async fn write_message(&mut self, msg: &str) -> io::Result<()> {
        match self {
            Self::Server { writer, .. } => {
                writer.write_all(msg.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
            },
            Self::Client { writer, .. } => {
                writer.write_all(msg.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
            },
        }
        Ok(())
    }
}

pub struct IpcListener {
    path: PathBuf,
}
impl IpcListener {
    pub async fn accept(&self) -> io::Result<IpcStream> {
        let server = ServerOptions::new()
            .first_pipe_instance(false)
            .create(&self.path)?;
        server.connect().await?;
        Ok(IpcStream::from_server(server))
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
}

pub fn server_listen(path: impl AsRef<Path>) -> io::Result<IpcListener> {
    // 实际 pipe instance 在 accept() 中创建并保持到连接完成，避免创建后立即 drop 的竞态。
    Ok(IpcListener {
        path: path.as_ref().to_path_buf(),
    })
}

pub async fn client_connect(path: impl AsRef<Path>) -> io::Result<IpcStream> {
    let client = ClientOptions::new().open(path.as_ref())?;
    Ok(IpcStream::from_client(client))
}
