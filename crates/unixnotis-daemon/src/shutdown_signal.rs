//! Signal handling for graceful shutdown
//!
//! Centralizes signal waiting logic used by the daemon runtime

use tokio::signal;
use tracing::warn;

pub(super) async fn shutdown_signal() {
    let ctrl_c = signal::ctrl_c();

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(err) => {
                warn!(?err, "failed to register SIGTERM handler");
                // Keep the future pending so startup does not abort on registration failure
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    #[tokio::test]
    async fn shutdown_signal_waits_when_no_signal_arrives() {
        let result =
            tokio::time::timeout(Duration::from_millis(25), super::shutdown_signal()).await;

        assert!(result.is_err());
    }
}
