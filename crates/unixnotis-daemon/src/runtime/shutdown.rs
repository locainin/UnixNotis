//! Signal handling for graceful shutdown
//!
//! Centralizes signal waiting logic used by the daemon runtime

use tokio::signal;
use tracing::warn;

pub async fn shutdown_signal() {
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
        () = terminate => {},
    }
}
