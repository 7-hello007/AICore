//! Owned, disposable Handler processes. Same-user resource containment, not
//! an adversarial filesystem/network sandbox. No installation is involved.
use crate::{ClientIdentity, IpcHandler};
use aicore_contracts::{
    ipc::protocol::{Envelope, IpcError, Request, ResponseData, ResponseEnvelope},
    plugin::host::{PluginProcessHandle, PluginProcessSpec},
};
use aicore_plugin::SubprocessPluginHost;
use std::sync::Arc;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ProcessHandlerRequest {
    pub envelope: Envelope,
    /// Created by the authenticated parent transport, never by wire clients.
    pub identity: ClientIdentity,
    #[serde(default)]
    pub state: serde_json::Value,
}

#[derive(Clone)]
pub struct ProcessHandlerSpec {
    pub process: PluginProcessSpec,
    pub args: Vec<String>,
}

pub struct ProcessIpcHandler {
    spec: ProcessHandlerSpec,
}
impl ProcessIpcHandler {
    pub fn new(spec: PluginProcessSpec) -> Result<Self, IpcError> {
        spec.validate()
            .map_err(|e| IpcError::invalid_request(e.to_string()))?;
        Ok(Self {
            spec: ProcessHandlerSpec {
                process: spec,
                args: Vec::new(),
            },
        })
    }
}
impl IpcHandler for ProcessIpcHandler {
    fn handle(&self, _: Request) -> Result<ResponseData, IpcError> {
        Err(IpcError::not_supported(
            "process handler requires managed dispatch",
        ))
    }
    fn process_spec(&self) -> Option<ProcessHandlerSpec> {
        Some(self.spec.clone())
    }
    fn name(&self) -> &'static str {
        "managed_process_handler"
    }
}

struct RunningProcess {
    host: Arc<SubprocessPluginHost>,
    handle: PluginProcessHandle,
}

/// A handshake may select an offered implemented version different from its
/// envelope. Every other response remains bound to the negotiated request.
pub(crate) fn response_matches(request: &Envelope, response: &serde_json::Value) -> bool {
    if response["request_id"].as_str() != Some(request.request_id.as_str()) {
        return false;
    }
    if let Request::Handshake {
        supported_versions, ..
    } = &request.payload
    {
        if response["success"] == true {
            let offered = if supported_versions.is_empty() {
                std::slice::from_ref(&request.version)
            } else {
                supported_versions
            };
            let Some(selected) = aicore_contracts::ipc::protocol::negotiate_version(offered) else {
                return false;
            };
            return response["version"] == serde_json::json!(selected)
                && response["data"]["kind"] == "handshake_ack"
                && response["data"]["selected_version"] == serde_json::json!(selected);
        }
        // The existing negotiation error uses the server's current version
        // when the peer offers no implemented wire format.
        if response["success"] == false && response["error"]["code"] == "ERR_PROTOCOL_MISMATCH" {
            return response["version"]
                == serde_json::json!(aicore_contracts::ipc::protocol::IPC_PROTOCOL_VERSION);
        }
    }
    response["version"] == serde_json::json!(request.version)
}
impl Drop for RunningProcess {
    fn drop(&mut self) {
        self.host.abort(&self.handle);
    }
}

pub(crate) async fn dispatch(
    spec: ProcessHandlerSpec,
    request: ProcessHandlerRequest,
    handler: Arc<dyn IpcHandler>,
    permit: tokio::sync::OwnedSemaphorePermit,
    deadline: tokio::time::Instant,
) -> ResponseEnvelope {
    let id = request.envelope.request_id.clone();
    let error = |code: &str, message: String| {
        ResponseEnvelope::err(id.clone(), IpcError::new(code, message))
    };
    if tokio::time::Instant::now() >= deadline {
        return error(
            "ERR_TIMEOUT",
            "handler deadline expired before start".into(),
        );
    }
    let json = match serde_json::to_string(&request) {
        Ok(json) => json,
        Err(e) => return error("ERR_INVALID_REQUEST", e.to_string()),
    };
    if json.len() >= crate::transport::MAX_MESSAGE_BYTES {
        return error(
            "ERR_INVALID_REQUEST",
            "managed request exceeds IPC frame ceiling".into(),
        );
    }
    let lease = Arc::new(permit);
    let host =
        Arc::new(SubprocessPluginHost::new().with_line_limit(crate::transport::MAX_MESSAGE_BYTES));
    let handle = match host.spawn_with_args(&spec.process, &spec.args) {
        Ok(handle) => handle,
        Err(e) => return error("ERR_RESOURCE_UNAVAILABLE", e.to_string()),
    };
    let running = RunningProcess {
        host: host.clone(),
        handle: handle.clone(),
    };
    let worker_lease = lease.clone();
    if tokio::time::Instant::now() >= deadline {
        drop(running);
        return error(
            "ERR_TIMEOUT",
            "handler deadline expired during process creation; child reaped".into(),
        );
    }
    let mut worker = tokio::task::spawn_blocking(move || {
        // Capacity remains reserved until pipe IO actually exits.
        let _permit = worker_lease;
        host.call_json(&handle, &json)
    });
    let result = match tokio::time::timeout_at(deadline, &mut worker).await {
        Ok(result) => result,
        Err(_) => {
            drop(running); // OS termination + reap + pipe-worker joins
            let _ = worker.await;
            return error(
                "ERR_TIMEOUT",
                "handler process terminated and reaped".into(),
            );
        },
    };
    drop(running);
    // `lease` remains owned by this future through process cleanup and the
    // bounded state commit; cancelled queued IO retains its worker clone.
    let _lease = lease;
    match result {
        Ok(Ok(json)) => match serde_json::from_str::<serde_json::Value>(&json) {
            Ok(response) if response_matches(&request.envelope, &response) => {
                if tokio::time::Instant::now() >= deadline {
                    return error(
                        "ERR_TIMEOUT",
                        "handler terminated; deadline expired before state commit".into(),
                    );
                }
                handler
                    .commit_process_response(
                        &request.envelope,
                        request.identity,
                        &request.state,
                        response,
                        deadline.into_std(),
                    )
                    .unwrap_or_else(|e| ResponseEnvelope::err(id, e))
            },
            _ => error(
                "ERR_INVALID_RESPONSE",
                "handler returned invalid response identity/schema".into(),
            ),
        },
        Ok(Err(aicore_contracts::errors::AicoreError::Timeout(message))) => error(
            "ERR_TIMEOUT",
            format!("handler process terminated and reaped: {message}"),
        ),
        Ok(Err(e)) => error("ERR_HANDLER_FAILED", e.to_string()),
        Err(e) => error("ERR_HANDLER_FAILED", e.to_string()),
    }
}

/// Worker entry point for a dedicated executable. One request per process;
/// stateful workers must receive state through their explicit broker protocol.
pub fn serve_once(handler: Arc<dyn IpcHandler>) -> std::io::Result<()> {
    let request = read_process_request()?;
    let response = crate::server::process_envelope(&handler, request.envelope, request.identity);
    write_process_response(&serde_json::to_value(response)?)
}

pub fn read_process_request() -> std::io::Result<ProcessHandlerRequest> {
    use std::io::{BufRead, Read};
    let mut bytes = Vec::new();
    let limit = crate::transport::MAX_MESSAGE_BYTES;
    let count = std::io::stdin()
        .lock()
        .take((limit + 1) as u64)
        .read_until(b'\n', &mut bytes)?;
    if count == 0 || count > limit || bytes.last() != Some(&b'\n') {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "invalid handler frame",
        ));
    }
    serde_json::from_slice(&bytes)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

pub fn write_process_response(response: &serde_json::Value) -> std::io::Result<()> {
    use std::io::Write;
    let json = serde_json::to_vec(response)?;
    if json.len() >= crate::transport::MAX_MESSAGE_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "handler response too large",
        ));
    }
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&json)?;
    stdout.write_all(b"\n")?;
    stdout.flush()
}

#[cfg(all(test, any(windows, target_os = "linux")))]
mod tests {
    use super::*;
    use std::{path::PathBuf, process::Command, sync::OnceLock, time::Duration};

    #[test]
    fn managed_handshake_preserves_negotiation_and_rejection_versions() {
        use aicore_contracts::{
            ipc::protocol::{IPC_PROTOCOL_V2, IPC_PROTOCOL_VERSION},
            schema::SchemaVersion,
        };
        let request = Envelope::new(
            "versions",
            Request::Handshake {
                capabilities: vec![],
                supported_versions: vec![IPC_PROTOCOL_VERSION, IPC_PROTOCOL_V2],
            },
        );
        let mut response = ResponseEnvelope::ok(
            "versions",
            ResponseData::HandshakeAck {
                capabilities: vec![],
                selected_version: IPC_PROTOCOL_V2,
            },
        );
        response.version = IPC_PROTOCOL_V2;
        assert!(response_matches(
            &request,
            &serde_json::to_value(&response).unwrap()
        ));
        response.request_id = "different".into();
        assert!(!response_matches(
            &request,
            &serde_json::to_value(response).unwrap()
        ));
        let mut rejected = Envelope::new(
            "rejected",
            Request::Handshake {
                capabilities: vec![],
                supported_versions: vec![SchemaVersion::new(99, 0)],
            },
        );
        rejected.version = SchemaVersion::new(99, 0);
        let response = ResponseEnvelope::err(
            "rejected",
            IpcError::protocol_mismatch(rejected.version, IPC_PROTOCOL_VERSION),
        );
        assert!(response_matches(&rejected,&serde_json::to_value(response).unwrap()),"must preserve the server's explicit protocol mismatch, not replace it with an invalid-response error");
    }

    const FIXTURE: &str = r#"
use std::io::{self, BufRead, Write};
fn main() {
    std::thread::spawn(|| { std::thread::sleep(std::time::Duration::from_secs(8)); std::process::exit(91); });
    let path = std::env::current_exe().unwrap();
    let mode = path.file_stem().unwrap().to_string_lossy();
    let line = io::stdin().lock().lines().next().unwrap().unwrap();
    std::fs::write(path.with_extension("pid"), std::process::id().to_string()).unwrap();
    let id = line.split("\"request_id\":\"").nth(1).unwrap().split('"').next().unwrap();
    if mode.contains("gate") { while !path.with_extension("gate").exists() { std::thread::sleep(std::time::Duration::from_millis(1)); } }
    if mode.contains("blocked") && !line.contains("\"kind\":\"handshake\"") { std::thread::sleep(std::time::Duration::from_secs(7)); }
    if mode.starts_with("crash") { std::process::exit(42); }
    let data = if line.contains("\"kind\":\"handshake\"") { r#X#{"kind":"handshake_ack","capabilities":[],"selected_version":"1.0"}#X# } else { r#X#{"kind":"ack"}#X# };
    println!("{{\"version\":\"1.0\",\"request_id\":\"{}\",\"success\":true,\"data\":{}}}", id, data);
    io::stdout().flush().unwrap();
    std::fs::write(path.with_extension("done"), "1").unwrap();
    if mode.contains("retain") { std::thread::sleep(std::time::Duration::from_secs(7)); }
}
"#;

    fn fixture(mode: &str) -> PluginProcessSpec {
        static DIRECTORY: OnceLock<PathBuf> = OnceLock::new();
        let directory = DIRECTORY.get_or_init(|| {
            let directory =
                std::env::temp_dir().join(format!("aicore-handler-tests-{}", std::process::id()));
            std::fs::create_dir_all(&directory).unwrap();
            let source = directory.join("fixture.rs");
            std::fs::write(
                &source,
                FIXTURE.replace("r#X#", "r#\"").replace("#X#", "\"#"),
            )
            .unwrap();
            let result = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
                .arg("--edition=2021")
                .arg(&source)
                .arg("-o")
                .arg(directory.join("fixture.exe"))
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            directory
        });
        let path = directory.join(format!("{mode}.exe"));
        std::fs::copy(directory.join("fixture.exe"), &path).unwrap();
        PluginProcessSpec {
            plugin_id: mode.into(),
            artifact_path: path.to_string_lossy().into(),
            timeout_ms: 2000,
            memory_limit_mb: 16,
        }
    }
    #[cfg(windows)]
    fn running(pid: u32) -> bool {
        use windows_sys::Win32::{
            Foundation::CloseHandle,
            System::Threading::{
                GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
            },
        };
        // SAFETY: query only the exact fixture PID and close the owned handle.
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process.is_null() {
                return false;
            }
            let mut code = 0;
            let ok = GetExitCodeProcess(process, &mut code);
            CloseHandle(process);
            assert_ne!(ok, 0);
            code == 259
        }
    }
    #[cfg(target_os = "linux")]
    fn running(pid: u32) -> bool {
        std::path::Path::new(&format!("/proc/{pid}")).exists()
    }

    fn endpoint(label: &str) -> PathBuf {
        #[cfg(windows)]
        {
            PathBuf::from(format!(
                r"\\.\pipe\aicore-managed-{}-{label}",
                std::process::id()
            ))
        }
        #[cfg(target_os = "linux")]
        {
            std::env::temp_dir().join(format!(
                "aicore-managed-{}-{label}.sock",
                std::process::id()
            ))
        }
    }
    async fn started(spec: &PluginProcessSpec) -> u32 {
        let path = PathBuf::from(&spec.artifact_path).with_extension("pid");
        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        loop {
            if let Ok(text) = std::fs::read_to_string(&path) {
                if let Ok(pid) = text.parse() {
                    return pid;
                }
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "handler did not start"
            );
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    }
    async fn call(
        spec: PluginProcessSpec,
        slots: Arc<tokio::sync::Semaphore>,
        ms: u64,
    ) -> ResponseEnvelope {
        let handler: Arc<dyn IpcHandler> = Arc::new(ProcessIpcHandler::new(spec).unwrap());
        crate::server::dispatch_blocking(
            handler,
            Envelope::new("managed", Request::GetStatus),
            ClientIdentity::Internal,
            slots,
            tokio::time::Instant::now() + Duration::from_millis(ms),
        )
        .await
    }
    #[tokio::test]
    async fn blocked_process_is_dead_and_capacity_released_before_timeout_response() {
        let spec = fixture("blocked-timeout");
        let slots = Arc::new(tokio::sync::Semaphore::new(1));
        let response = call(spec.clone(), slots.clone(), 500).await;
        let pid = started(&spec).await;
        assert_eq!(response.error.unwrap().code, "ERR_TIMEOUT");
        assert!(!running(pid), "caller timeout left Handler process alive");
        assert_eq!(slots.available_permits(), 1);
    }
    #[tokio::test]
    async fn cancelled_request_reaps_process_and_repeated_calls_reuse_capacity() {
        let spec = fixture("blocked-disconnect");
        let slots = Arc::new(tokio::sync::Semaphore::new(1));
        let task = tokio::spawn(call(spec.clone(), slots.clone(), 5000));
        let pid = started(&spec).await;
        assert!(running(pid));
        let busy = call(spec.clone(), slots.clone(), 500).await;
        assert_eq!(busy.error.unwrap().code, "ERR_RESOURCE_UNAVAILABLE");
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert!(!running(pid));
        let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
        while slots.available_permits() == 0 {
            assert!(tokio::time::Instant::now() < deadline);
            tokio::task::yield_now().await;
        }
        let healthy = fixture("healthy-repeat");
        for _ in 0..8 {
            assert!(call(healthy.clone(), slots.clone(), 1000).await.success);
            assert_eq!(slots.available_permits(), 1);
        }
    }
    #[tokio::test]
    async fn crashing_handler_does_not_kill_host() {
        let slots = Arc::new(tokio::sync::Semaphore::new(1));
        let spec = fixture("crash-handler");
        assert_eq!(
            call(spec.clone(), slots.clone(), 1000)
                .await
                .error
                .unwrap()
                .code,
            "ERR_HANDLER_FAILED"
        );
        assert!(!running(started(&spec).await));
        assert!(
            call(fixture("healthy-after-crash"), slots, 1000)
                .await
                .success
        );
    }

    async fn connection_cleanup(shutdown_server: bool) {
        let spec = fixture(if shutdown_server {
            "blocked-server-shutdown"
        } else {
            "blocked-client-disconnect"
        });
        let handler = Arc::new(ProcessIpcHandler::new(spec.clone()).unwrap());
        let path = endpoint(if shutdown_server {
            "shutdown"
        } else {
            "disconnect"
        });
        let (shutdown, rx) = tokio::sync::mpsc::channel(1);
        let server = crate::IpcServer::bind(&path, handler)
            .unwrap()
            .with_shutdown(rx);
        let task = tokio::spawn(server.run());
        let mut client = crate::IpcClient::connect(&path).await.unwrap();
        std::fs::remove_file(PathBuf::from(&spec.artifact_path).with_extension("pid")).unwrap();
        let request = tokio::spawn(async move {
            client
                .call(
                    "managed-blocked",
                    Request::GetStatus,
                    Duration::from_secs(10),
                )
                .await
        });
        let pid = started(&spec).await;
        assert!(running(pid));
        if shutdown_server {
            shutdown.send(()).await.unwrap();
            tokio::time::timeout(Duration::from_secs(1), task)
                .await
                .unwrap()
                .unwrap();
            assert!(request.await.unwrap().is_err());
        } else {
            request.abort();
            let _ = request.await;
            let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
            while running(pid) {
                assert!(
                    tokio::time::Instant::now() < deadline,
                    "disconnect left Handler alive"
                );
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
            shutdown.send(()).await.unwrap();
            task.await.unwrap();
        }
        assert!(!running(pid), "shutdown left Handler alive");
    }
    #[tokio::test]
    async fn client_disconnect_terminates_handler_process() {
        connection_cleanup(false).await;
    }
    #[tokio::test]
    async fn server_shutdown_terminates_handler_process() {
        connection_cleanup(true).await;
    }

    #[tokio::test]
    async fn client_timeout_closes_transport_and_terminates_process_without_client_drop() {
        let spec = fixture("blocked-client-timeout");
        let path = endpoint("timeout");
        let (shutdown, rx) = tokio::sync::mpsc::channel(1);
        let server = crate::IpcServer::bind(
            &path,
            Arc::new(ProcessIpcHandler::new(spec.clone()).unwrap()),
        )
        .unwrap()
        .with_shutdown(rx);
        let server = tokio::spawn(server.run());
        let mut client = crate::IpcClient::connect(path).await.unwrap();
        std::fs::remove_file(PathBuf::from(&spec.artifact_path).with_extension("pid")).unwrap();
        let request = tokio::spawn(async move {
            let result = client
                .call(
                    "client-timeout",
                    Request::GetStatus,
                    Duration::from_millis(300),
                )
                .await;
            (client, result)
        });
        let pid = started(&spec).await;
        let (mut client, result) = request.await.unwrap();
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::TimedOut);
        let deadline = tokio::time::Instant::now() + Duration::from_millis(700);
        while running(pid) && tokio::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        let terminated = !running(pid);
        shutdown.send(()).await.unwrap();
        server.await.unwrap();
        assert!(client
            .call("reuse", Request::GetStatus, Duration::from_millis(100))
            .await
            .is_err());
        assert!(
            terminated,
            "client timeout kept transport and Handler process alive"
        );
    }

    #[tokio::test]
    async fn duplicate_request_is_rejected_without_launching_another_process() {
        let spec = fixture("healthy-duplicate");
        let path = endpoint("duplicate");
        let (shutdown, rx) = tokio::sync::mpsc::channel(1);
        let server = crate::IpcServer::bind(
            &path,
            Arc::new(ProcessIpcHandler::new(spec.clone()).unwrap()),
        )
        .unwrap()
        .with_shutdown(rx);
        let server = tokio::spawn(server.run());
        let mut client = crate::IpcClient::connect(path).await.unwrap();
        assert!(
            client
                .call("repeat", Request::GetStatus, Duration::from_secs(1))
                .await
                .unwrap()
                .success
        );
        let pid_file = PathBuf::from(&spec.artifact_path).with_extension("pid");
        let before = std::fs::read_to_string(&pid_file).unwrap();
        let response = client
            .call("repeat", Request::GetStatus, Duration::from_secs(1))
            .await
            .unwrap();
        assert_eq!(response.error.unwrap().code, "ERR_INVALID_REQUEST");
        assert_eq!(std::fs::read_to_string(pid_file).unwrap(), before);
        shutdown.send(()).await.unwrap();
        server.await.unwrap();
    }

    #[tokio::test]
    async fn unsupported_limit_is_rejected_before_handler_entry_point() {
        let mut spec = fixture("healthy-unsupported-limit");
        spec.memory_limit_mb = u64::MAX;
        let pid_file = PathBuf::from(&spec.artifact_path).with_extension("pid");
        assert!(!pid_file.exists());
        let slots = Arc::new(tokio::sync::Semaphore::new(1));
        let response = call(spec, slots.clone(), 1000).await;
        assert_eq!(response.error.unwrap().code, "ERR_RESOURCE_UNAVAILABLE");
        assert!(
            !pid_file.exists(),
            "unenforceable containment ran Handler code"
        );
        assert_eq!(slots.available_permits(), 1);
    }

    #[tokio::test]
    async fn completed_pipe_io_keeps_capacity_until_process_cleanup() {
        use std::{future::Future, task::Poll};
        let spec = fixture("reply-gate-retain");
        let slots = Arc::new(tokio::sync::Semaphore::new(1));
        let mut future = Box::pin(call(spec.clone(), slots.clone(), 3000));
        std::future::poll_fn(|cx| {
            assert!(future.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        })
        .await;
        let pid = started(&spec).await;
        let path = PathBuf::from(&spec.artifact_path);
        std::fs::write(path.with_extension("gate"), "1").unwrap();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
        while !path.with_extension("done").exists() {
            assert!(tokio::time::Instant::now() < deadline);
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert!(running(pid));
        assert_eq!(
            slots.available_permits(),
            0,
            "capacity released while process and pipe workers still owned by an unpolled request"
        );
        assert!(future.await.success);
        assert!(!running(pid));
        assert_eq!(slots.available_permits(), 1);
    }
}
