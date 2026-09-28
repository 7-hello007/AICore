use tokio::sync::mpsc;

/// Commencing signal listening.
///
/// Monitor Ctrl+C (SIGINT) and SIGTERM (Linux/Unix).
/// Send an `()` to `tx` after receiving the signal.
pub fn spawn_signal_listener(tx: mpsc::Sender<()>) {
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};

            let mut sigint = match signal(SignalKind::interrupt()) {
                Ok(s) => s,
                Err(_) => {
                    // Back to ctrl_c
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
