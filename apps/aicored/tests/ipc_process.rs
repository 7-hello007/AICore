//! Real daemon IPC in model mode. No installation or native hardware control.
use aicore_contracts::{
    application::id::ApplicationId,
    ipc::protocol::{
        Request, ResponseData, ResponseEnvelope, IPC_PROTOCOL_V2, IPC_PROTOCOL_VERSION,
    },
    task::contract::TaskContract,
};
use aicore_ipc::IpcClient;
use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

struct Daemon {
    child: Child,
    scratch: PathBuf,
    endpoint: PathBuf,
}
impl Drop for Daemon {
    fn drop(&mut self) {
        // The process owns all request Jobs/groups; completed requests have
        // already been reaped before their response. Never install the binary.
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.scratch);
    }
}
fn launch() -> Daemon {
    let scratch =
        std::env::temp_dir().join(format!("aicore-daemon-process-{}", std::process::id()));
    std::fs::create_dir_all(scratch.join("plugins")).unwrap();
    #[cfg(windows)]
    let endpoint = PathBuf::from(format!(
        r"\\.\pipe\aicore-daemon-process-{}",
        std::process::id()
    ));
    #[cfg(target_os = "linux")]
    let endpoint = scratch.join("daemon.sock");
    let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let digest = aicore_plugin::SignatureVerifier::compute_sha256(std::env::current_exe().unwrap())
        .unwrap()
        .hash_hex;
    std::fs::write(scratch.join("pins.json"), serde_json::to_vec(&serde_json::json!([{
        "application_id":"managed.app", "artifact_sha256":digest, "allowed_actions":[
            "register","unregister","submit_task","declare_task","clear_task",
            "submit_native_task","read_native_task","cancel_native_task","propose_intent","prepare_task"
        ]
    }])).unwrap()).unwrap();
    let child = Command::new(env!("CARGO_BIN_EXE_aicored"))
        .args([
            "--config",
            scratch.join("config.json").to_str().unwrap(),
            "--state-path",
            scratch.join("status.json").to_str().unwrap(),
            "--memory-path",
            scratch.join("memory.db").to_str().unwrap(),
            "--plugins-dir",
            scratch.join("plugins").to_str().unwrap(),
            "--ipc-path",
            endpoint.to_str().unwrap(),
            "--interval-ms",
            "50",
        ])
        .env("AICORE_EXECUTION_MODE", "model")
        .env("AICORE_PROJECT_ROOT", &project)
        .env("AICORE_APP_POLICY_PATH", scratch.join("pins.json"))
        .env("AICORE_INTENT_BACKEND", "json")
        .env(
            "AICORE_INTENT_MODEL_PATH",
            project.join("models/intent_classifier_v1.json"),
        )
        .env("TOKIO_WORKER_THREADS", "2")
        .env("TEMP", &scratch)
        .env("TMP", &scratch)
        .env("TMPDIR", &scratch)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    Daemon {
        child,
        scratch,
        endpoint,
    }
}
async fn query(client: &mut IpcClient, id: &str, request: Request) -> ResponseEnvelope {
    client
        .call(id, request, Duration::from_secs(10))
        .await
        .unwrap()
}
async fn accepted(client: &mut IpcClient, id: &str, request: Request) -> ResponseData {
    let response = query(client, id, request).await;
    assert!(response.success, "{id}: {:?}", response.error);
    response.data.unwrap()
}

#[tokio::test]
async fn production_daemon_uses_managed_protocol_and_acknowledges_live_state() {
    let mut daemon = launch();
    let limit = Instant::now() + Duration::from_secs(15);
    let mut client = loop {
        match IpcClient::connect(&daemon.endpoint).await {
            Ok(client) => break client,
            Err(error) => {
                assert!(
                    daemon.child.try_wait().unwrap().is_none(),
                    "daemon exited: {error}"
                );
                assert!(
                    Instant::now() < limit,
                    "daemon did not become ready: {error}"
                );
                tokio::time::sleep(Duration::from_millis(30)).await;
            },
        }
    };
    // A 1.0 envelope may negotiate 2.0. The process boundary must retain this.
    let mut v2 = IpcClient::connect_with_versions(
        &daemon.endpoint,
        vec![IPC_PROTOCOL_VERSION, IPC_PROTOCOL_V2],
    )
    .await
    .unwrap();
    let versioned = query(&mut v2, "v2-query", Request::GetStatus).await;
    assert!(versioned.success, "{:?}", versioned.error);
    assert_eq!(versioned.version, IPC_PROTOCOL_V2);
    let app = ApplicationId("managed.app".into());
    let register = |id| Request::Register {
        application_id: id,
        name: "managed".into(),
        version: "1".into(),
        capabilities: vec![],
    };
    let denied = query(
        &mut client,
        "unpinned",
        register(ApplicationId("unapproved.app".into())),
    )
    .await;
    assert_eq!(denied.error.unwrap().code, "ERR_PERMISSION_DENIED");
    assert!(
        matches!(accepted(&mut client, "register", register(app.clone())).await, ResponseData::Registered { application_id } if application_id == app.0)
    );
    let mut contract = TaskContract::minimal(app.clone());
    contract.task_id = Some("managed-explicit-task".into());
    assert!(matches!(
        accepted(
            &mut client,
            "declare",
            Request::DeclareCurrentTask {
                contract: contract.clone()
            }
        )
        .await,
        ResponseData::CurrentTaskDeclared
    ));
    assert!(matches!(
        accepted(
            &mut client,
            "clear",
            Request::ClearCurrentTask {
                application_id: app.clone()
            }
        )
        .await,
        ResponseData::CurrentTaskCleared { cleared: true }
    ));
    assert!(
        matches!(accepted(&mut client, "submit", Request::SubmitTask { contract:contract.clone() }).await, ResponseData::TaskSubmitted { accepted:true, task_id:Some(id), .. } if id == "managed-explicit-task")
    );
    let duplicate = query(
        &mut client,
        "submit-again",
        Request::SubmitTask {
            contract: contract.clone(),
        },
    )
    .await;
    assert!(!duplicate.success, "duplicate state write acknowledged");
    contract.task_id = Some("managed-native-task".into());
    let ResponseData::NativeTaskSubmitted { handle_id } = accepted(
        &mut client,
        "native",
        Request::SubmitNativeTask {
            contract: contract.clone(),
        },
    )
    .await
    else {
        panic!("missing native handle")
    };
    assert!(matches!(
        accepted(
            &mut client,
            "read-native",
            Request::GetNativeTask {
                application_id: app.clone(),
                handle_id: handle_id.clone()
            }
        )
        .await,
        ResponseData::NativeTaskStatus { .. }
    ));
    // Another process with the same pinned executable must not inherit ownership.
    let mut impostor = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "foreign_peer_cannot_revoke_binding",
            "--nocapture",
        ])
        .env("AICORE_TEST_FOREIGN_ENDPOINT", &daemon.endpoint)
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let status = impostor.wait().unwrap();
    assert!(status.success(), "foreign process ownership regression");
    let preparation = query(
        &mut client,
        "consent",
        Request::PrepareUpcomingTask {
            contract: contract.clone(),
            starts_at_unix_ms: now_ms() + 30_000,
        },
    )
    .await;
    assert_eq!(preparation.error.unwrap().code, "ERR_PERMISSION_DENIED");
    // Reopen the same SQLite file used by the running daemon. The child must
    // read current consent/history, rather than an unrelated in-memory copy.
    use aicore_contracts::{
        context::workload::{InteractionLevel, WorkloadType},
        execution::status::ExecutionOverallStatus,
        memory::{profile::UserProfile, record::HistoricalRecord},
        policy::{priority::PolicySource, resource_policy::PowerMode},
        traits::memory_store::IMemoryStore,
    };
    let memory = aicore_memory::SqliteMemoryStore::open(daemon.scratch.join("memory.db")).unwrap();
    memory
        .save_profile(&UserProfile {
            profile_enabled: true,
            ..UserProfile::default()
        })
        .unwrap();
    let starts = now_ms() + 60_000;
    for day in 1..=3 {
        memory
            .append_record(&HistoricalRecord {
                record_id: format!("process-pattern-{day}"),
                application_id: Some(app.clone()),
                timestamp_unix_ms: (starts / 3_600_000) * 3_600_000 - day * 86_400_000,
                workload_type: WorkloadType::Development,
                interaction_level: InteractionLevel::Interactive,
                policy_id: "prior-model".into(),
                policy_source: PolicySource::RuleBased,
                overall_status: ExecutionOverallStatus::AllSucceeded,
                cpu_usage_percent: 1.0,
                gpu_usage_percent: None,
                npu_usage_percent: None,
                memory_usage_percent: 1.0,
                power_mode: PowerMode::Balanced,
            })
            .unwrap();
    }
    let ResponseData::PreparationScheduled {
        preparation_id,
        workload,
        ..
    } = accepted(
        &mut client,
        "prepare-shared",
        Request::PrepareUpcomingTask {
            contract: contract.clone(),
            starts_at_unix_ms: starts,
        },
    )
    .await
    else {
        panic!("missing preparation ack")
    };
    assert!(!preparation_id.is_empty());
    assert_eq!(workload, WorkloadType::Development);
    memory.save_profile(&UserProfile::default()).unwrap();
    let revoked = query(
        &mut client,
        "revoked-consent",
        Request::PrepareUpcomingTask {
            contract: contract.clone(),
            starts_at_unix_ms: starts,
        },
    )
    .await;
    assert_eq!(revoked.error.unwrap().code, "ERR_PERMISSION_DENIED");
    let intent = accepted(
        &mut client,
        "json-intent",
        Request::ProposeIntent {
            request: aicore_contracts::ai::intent::IntentRequest {
                application_id: app.clone(),
                text: "compile rust project".into(),
                submitted_at_unix_ms: now_ms(),
            },
        },
    )
    .await;
    assert!(
        matches!(intent,ResponseData::IntentProposed { proposal } if proposal.expires_at_unix_ms>now_ms())
    );
    for (id, request) in [
        ("context", Request::GetContext),
        ("policy", Request::GetPolicy),
        ("ticks", Request::GetRecentTicks { limit: 3 }),
        ("plugins", Request::ListPlugins),
        ("dashboard", Request::GetDashboardSnapshot),
    ] {
        accepted(&mut client, id, request).await;
    }
    let ResponseData::Status { status: first } =
        accepted(&mut client, "status1", Request::GetStatus).await
    else {
        panic!()
    };
    tokio::time::sleep(Duration::from_millis(100)).await;
    let ResponseData::Status { status: last } =
        accepted(&mut client, "status2", Request::GetStatus).await
    else {
        panic!()
    };
    assert!(
        last.tick_count > first.tick_count,
        "stale daemon state snapshot"
    );
    assert!(matches!(
        accepted(
            &mut client,
            "unregister",
            Request::Unregister {
                application_id: app
            }
        )
        .await,
        ResponseData::Unregistered { removed: true }
    ));
    drop(memory);
    drop(client);
    drop(v2);
}
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
#[tokio::test]
async fn foreign_peer_cannot_revoke_binding() {
    let Some(endpoint) = std::env::var_os("AICORE_TEST_FOREIGN_ENDPOINT") else {
        return;
    };
    let mut client = IpcClient::connect(PathBuf::from(endpoint)).await.unwrap();
    let response = query(
        &mut client,
        "foreign",
        Request::Unregister {
            application_id: ApplicationId("managed.app".into()),
        },
    )
    .await;
    assert_eq!(response.error.unwrap().code, "ERR_PERMISSION_DENIED");
}
