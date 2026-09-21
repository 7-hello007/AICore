use tokio::sync::mpsc;

/// 启动信号监听。
///
/// 监听 Ctrl+C（SIGINT）与 SIGTERM（Linux / Unix）。
/// 收到信号后向 `tx` 发送一个 `()`。
pub fn spawn_signal_listener(tx: mpsc::Sender<()>) {
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};

            let mut sigint = match signal(SignalKind::interrupt()) {
                Ok(s) => s,
                Err(_) => {
                    // 回退到 ctrl_c
                    let _ = tokio::signal::ctrl_c().await;
                    let _ = tx.send(()).await;
                    return;
                },
            };
            let mut sigterm = match signal(SignalKind::terminate()) {
                Ok(s) => s,
                Err(_) => {
                    let _ = tokio::signal::ctrl_c().await;
                    let _ = tx.send(()).await;
                    return;
                },
            };

            tokio::select! {
                _ = sigint.recv() => {},
                _ = sigterm.recv() => {},
            }
        }

        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }

        let _ = tx.send(()).await;
    });
}
