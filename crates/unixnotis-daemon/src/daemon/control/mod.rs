//! D-Bus server for com.unixnotis.Control

mod clear;
mod dnd;
mod inhibit;
mod panel;
mod query;
mod sanitize;
mod server;
mod watch;

pub use server::ControlServer;
pub async fn spawn_inhibitor_owner_watch(
    state: std::sync::Arc<crate::daemon::DaemonState>,
) -> zbus::Result<()> {
    watch::spawn_inhibitor_owner_watch(state).await
}

// Cap inhibitor count so memory use stays bounded even under abusive clients
const MAX_ACTIVE_INHIBITORS: u32 = 128;

#[cfg(test)]
mod tests;
