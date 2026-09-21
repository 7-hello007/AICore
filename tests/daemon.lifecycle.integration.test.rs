//! Daemon 生命周期集成测试。
//!
//! 通过子进程启动 `aicored --once`，验证：
//! - 进程退出码为 0
//! - 状态文件被写入且结构合法

use std::path::PathBuf;
use std::process::Command;

fn daemon_bin() -> PathBuf {
    // 测试运行时，编译好的二进制在 target/debug/aicored
    let mut path = std::env::current_exe().expect("current_exe");
    path.pop(); // 去掉测试文件名
    if path.ends_with("deps") {
        path.pop();
    }
    path.join("aicored")
}

#[test]
fn daemon_once_writes_state_file() {
    let bin = daemon_bin();
    if !bin.exists() {
        eprintln!("skip: {} not built", bin.display());
        return;
    }

    let tmp = std::env::temp_dir().join(format!("aicore-test-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    let state_path = tmp.join("status.json");
    let _ = std::fs::remove_file(&state_path);

    let status = Command::new(&bin)
        .arg("--once")
        .arg("--state-path")
        .arg(&state_path)
        .status()
        .expect("spawn aicored");

    assert!(status.success(), "aicored --once failed: {:?}", status);

    assert!(state_path.exists(), "status file not written");

    let content = std::fs::read_to_string(&state_path).expect("read state");
    let value: serde_json::Value = serde_json::from_str(&content).expect("parse state json");

    assert_eq!(value["version"], "1.0");
    assert!(value["pid"].is_u64());
    assert_eq!(value["tick_count"], 1);
    assert!(value["last_tick"].is_object());

    let _ = std::fs::remove_file(&state_path);
    let _ = std::fs::remove_dir(&tmp);
}
