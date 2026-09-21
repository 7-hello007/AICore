use std::sync::Arc;
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

use crate::AppState;

/// SSE 流：周期性推送 DashboardSnapshot JSON。
pub async fn stream(stream: &mut TcpStream, state: Arc<AppState>) -> std::io::Result<()> {
    // 建立 SSE 连接头
    let headers = "HTTP/1.1 200 OK\r\n\
                   Content-Type: text/event-stream\r\n\
                   Cache-Control: no-cache\r\n\
                   Connection: keep-alive\r\n\
\
                   X-Accel-Buffering: no\r\n\
                   \r\n";
    stream.write_all(headers.as_bytes()).await?;
    stream.flush().await?;

    // Reconnect is handled by EventSource; the server always sends a fresh current snapshot first.
    // No stale replay is attempted because the dashboard is a live-state view, not an event log.

    let mut rx = state.event_tx.subscribe();

    // 立即发一次当前快照
    let snap = state.client.fetch().await;
    if let Ok(json) = serde_json::to_string(&snap) {
        let id = state
            .event_seq
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let _ = write_event(stream, id, &json).await;
    }

    // 心跳间隔
    let mut heartbeat = tokio::time::interval(Duration::from_secs(15));
    heartbeat.tick().await;

    loop {
        tokio::select! {
            event = rx.recv() => {
                match event {
                    Ok(event) => {
                        if write_event(stream, event.id, &event.payload).await.is_err() {
                            return Ok(());
                        }
                    }
                    Err(_) => break,
                }
            }
            _ = heartbeat.tick() => {
                // SSE 注释行作为心跳
                if stream.write_all(b": keepalive\n\n").await.is_err() {
                    return Ok(());
                }
                let _ = stream.flush().await;
            }
        }
    }

    Ok(())
}

async fn write_event(stream: &mut TcpStream, id: u64, json: &str) -> std::io::Result<()> {
    // JSON 里可能有换行，需要每行加 `data:` 前缀；这里 JSON 是单行
    let payload = format!("id: {}\ndata: {}\n\n", id, json);
    stream.write_all(payload.as_bytes()).await?;
    stream.flush().await?;
    Ok(())
}
