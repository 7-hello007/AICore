use std::io;
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use async_trait::async_trait;
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::{
    ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
};

use super::{read_bounded_message, IpcTransport};
use crate::ClientIdentity;

type ServerRead = tokio::io::ReadHalf<NamedPipeServer>;
type ServerWrite = tokio::io::WriteHalf<NamedPipeServer>;
type ClientRead = tokio::io::ReadHalf<NamedPipeClient>;
type ClientWrite = tokio::io::WriteHalf<NamedPipeClient>;

pub enum IpcStream {
    Server {
        reader: BufReader<ServerRead>,
        writer: ServerWrite,
        identity: ClientIdentity,
    },
    Client {
        reader: BufReader<ClientRead>,
        writer: ClientWrite,
    },
}

impl IpcStream {
    fn from_server(stream: NamedPipeServer) -> Self {
        let mut pid = 0u32;
        // SAFETY: the connected pipe handle is valid for this call and `pid`
        // remains writable for its duration. Failure is unauthenticated.
        let identity = if unsafe {
            windows_sys::Win32::System::Pipes::GetNamedPipeClientProcessId(
                stream.as_raw_handle(),
                &mut pid,
            )
        } != 0
            && pid != 0
        {
            process_creation_time(pid)
                .map(|created_at_100ns| ClientIdentity::Windows {
                    pid,
                    created_at_100ns,
                })
                .unwrap_or(ClientIdentity::Unauthenticated)
        } else {
            ClientIdentity::Unauthenticated
        };
        let (read, write) = tokio::io::split(stream);
        Self::Server {
            reader: BufReader::new(read),
            writer: write,
            identity,
        }
    }
    fn from_client(stream: NamedPipeClient) -> Self {
        let (read, write) = tokio::io::split(stream);
        Self::Client {
            reader: BufReader::new(read),
            writer: write,
        }
    }

    pub fn peer_identity(&self) -> ClientIdentity {
        match self {
            Self::Server { identity, .. } => *identity,
            Self::Client { .. } => ClientIdentity::Unauthenticated,
        }
    }
}

pub(crate) fn process_creation_time(pid: u32) -> Option<u64> {
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME};
    use windows_sys::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    // SAFETY: PID comes from the kernel's pipe peer query. The returned
    // handle is closed on every path after successful OpenProcess.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return None;
    }
    let mut created: FILETIME = unsafe { std::mem::zeroed() };
    let mut exited: FILETIME = unsafe { std::mem::zeroed() };
    let mut kernel: FILETIME = unsafe { std::mem::zeroed() };
    let mut user: FILETIME = unsafe { std::mem::zeroed() };
    // SAFETY: all FILETIME outputs are valid writable buffers, and `process`
    // remains open for the call.
    let result =
        unsafe { GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user) };
    // SAFETY: this handle was returned by OpenProcess above.
    unsafe { CloseHandle(process) };
    (result != 0).then_some(((created.dwHighDateTime as u64) << 32) | created.dwLowDateTime as u64)
}

pub(crate) fn process_executable_path(pid: u32) -> Option<PathBuf> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    // SAFETY: OpenProcess returns a handle owned here and closed below.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return None;
    }
    let mut path = vec![0u16; 32_768];
    let mut length = path.len() as u32;
    // SAFETY: `path` is a writable UTF-16 buffer and `length` gives its size.
    let ok = unsafe { QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut length) };
    // SAFETY: this handle came from OpenProcess above.
    unsafe { CloseHandle(process) };
    (ok != 0 && length > 0)
        .then(|| PathBuf::from(String::from_utf16_lossy(&path[..length as usize])))
}

pub(crate) fn process_identity_stale(pid: u32, created_at_100ns: u64) -> bool {
    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, ERROR_INVALID_PARAMETER, WAIT_OBJECT_0,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, WaitForSingleObject, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    // SAFETY: PID originated from the pipe peer query; the returned handle is
    // closed on every successful-open path. Unknown access errors fail closed.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return unsafe { GetLastError() } == ERROR_INVALID_PARAMETER;
    }
    // SAFETY: the process handle is valid; zero timeout only queries state.
    let wait = unsafe { WaitForSingleObject(process, 0) };
    // SAFETY: this handle was returned by OpenProcess above.
    unsafe { CloseHandle(process) };
    wait == WAIT_OBJECT_0
        || process_creation_time(pid).is_some_and(|current| current != created_at_100ns)
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
    pending: Mutex<Option<NamedPipeServer>>,
}
impl IpcListener {
    pub async fn accept(&self) -> io::Result<IpcStream> {
        let server = self
            .pending
            .lock()
            .map_err(|_| io::Error::other("pipe listener lock poisoned"))?
            .take()
            .ok_or_else(|| io::Error::other("pipe listener has no pending instance"))?;
        let connected = server.connect().await;
        // Keep an owned pipe instance alive before handing this one to a
        // connection. Otherwise a local process could squat the pipe name in
        // the gap between clients.
        let next = create_owner_only_pipe(&self.path, false)?;
        *self
            .pending
            .lock()
            .map_err(|_| io::Error::other("pipe listener lock poisoned"))? = Some(next);
        connected?;
        Ok(IpcStream::from_server(server))
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn create_owner_only_pipe(path: &Path, first_instance: bool) -> io::Result<NamedPipeServer> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
    use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;

    // Protected DACL: only the creating object owner and LocalSystem get access.
    // Remote pipe clients are independently rejected by ServerOptions.
    let sddl: Vec<u16> = "D:P(A;;GA;;;OW)(A;;GA;;;SY)"
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut descriptor = std::ptr::null_mut();
    // SAFETY: the SDDL buffer is NUL terminated and `descriptor` is writable.
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut descriptor,
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let mut attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor,
        bInheritHandle: 0,
    };
    // SAFETY: `attributes` and its descriptor remain alive through pipe
    // creation. Windows copies the descriptor for the new pipe object.
    let result = unsafe {
        ServerOptions::new()
            .first_pipe_instance(first_instance)
            .reject_remote_clients(true)
            .create_with_security_attributes_raw(
                path,
                (&mut attributes as *mut SECURITY_ATTRIBUTES).cast(),
            )
    };
    // SAFETY: ConvertStringSecurityDescriptorToSecurityDescriptorW allocated
    // this descriptor with LocalAlloc, and it is no longer used.
    unsafe { LocalFree(descriptor) };
    result
}

pub fn server_listen(path: impl AsRef<Path>) -> io::Result<IpcListener> {
    let path = path.as_ref().to_path_buf();
    let first = create_owner_only_pipe(&path, true)?;
    Ok(IpcListener {
        path,
        pending: Mutex::new(Some(first)),
    })
}

pub async fn client_connect(path: impl AsRef<Path>) -> io::Result<IpcStream> {
    let client = ClientOptions::new().open(path.as_ref())?;
    Ok(IpcStream::from_client(client))
}
