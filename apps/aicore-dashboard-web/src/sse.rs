use std::sync::Arc;
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

use crate::AppState;

/// SSE current: Periodically transfer DashboardSnapshot JSON.
pub async fn stream(stream: &mut TcpStream, state: Arc<AppState>) -> std::io::Result<()> {
    // Create SSE connection header
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

    // Send the current snapshot immediately.
    let snap = state.client.fetch().await;
    if let Ok(json) = serde_json::to_string(&snap) {
        let id = state
            .event_seq
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let _ = write_event(stream, id, &json).await;
    }

    // Heart beat interval
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
                // SSE Comment Line as a heartbeat
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
    let mut payload = format!("id: {}\n", id);
    for line in json.lines() {
        payload.push_str("data: ");
        payload.push_str(line);
        payload.push('\n');
    }
    payload.push('\n');
    stream.write_all(payload.as_bytes()).await?;
    stream.flush().await?;
    Ok(())
}
