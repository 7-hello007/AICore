//! 示例插件进程：读取 stdin 的 JSON 行请求，输出 JSON 行响应。
//!
//! 协议（与 plugin::host::protocol 一致）：
//! - `{"kind":"ping"}` → `{"kind":"pong"}`
//! - `{"kind":"execute","input":N}` → `{"kind":"result","output":2N+1}`
//! - `{"kind":"shutdown"}` → `{"kind":"shutting_down"}` 并退出

use std::io::{BufRead, Write};

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Request {
    Ping,
    Execute { input: u32 },
    Shutdown,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Response {
    Pong,
    Result { output: u32 },
    ShuttingDown,
}

fn main() {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }

        let request: Request = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(_) => {
                let _ = writeln!(out, r#"{{"kind":"error","message":"bad request"}}"#);
                let _ = out.flush();
                continue;
            },
        };

        let response = match request {
            Request::Ping => Response::Pong,
            Request::Execute { input } => Response::Result {
                output: input.wrapping_mul(2).wrapping_add(1),
            },
            Request::Shutdown => {
                let _ = writeln!(
                    out,
                    "{}",
                    serde_json::to_string(&Response::ShuttingDown).unwrap()
                );
                let _ = out.flush();
                break;
            },
        };

        let _ = writeln!(out, "{}", serde_json::to_string(&response).unwrap());
        let _ = out.flush();
    }
}
