//! Cross-platform shutdown signal handling.

/// Wait for SIGTERM or SIGINT (unix) / Ctrl-C (windows).
pub async fn wait_for_shutdown_signal() {
    #[cfg(unix)]
    {
        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
        let sigint = tokio::signal::ctrl_c();
        tokio::select! {
            _ = sigterm.recv() => {
                eprintln!("[INFO] [runtime] Received SIGTERM");
            }
            _ = sigint => {
                eprintln!("[INFO] [runtime] Received SIGINT");
            }
        }
    }
    #[cfg(not(unix))]
    {
        // Windows: only CTRL_C is available
        let _ = tokio::signal::ctrl_c().await;
        eprintln!("[INFO] [runtime] Received Ctrl-C");
    }
}
