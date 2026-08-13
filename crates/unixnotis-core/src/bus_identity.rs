//! Sanitized session-bus identity diagnostics shared by every process

use tracing::info;
use zbus::fdo::DBusProxy;
use zbus::Connection;

use crate::INTERNAL_DBUS_CALL_TIMEOUT;

/// Stable identity assigned by one message-bus instance and connection
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionBusIdentity {
    pub bus_id: String,
    pub unique_name: String,
    pub runtime_dir: String,
}

/// Read and log a sanitized session-bus identity
///
/// # Errors
///
/// Returns an error when the bus identity probe fails, times out, or lacks a unique name
pub async fn log_session_bus_identity(
    connection: &Connection,
    component: &'static str,
) -> zbus::Result<SessionBusIdentity> {
    let dbus = DBusProxy::new(connection).await?;
    let bus_id = tokio::time::timeout(INTERNAL_DBUS_CALL_TIMEOUT, dbus.get_id())
        .await
        .map_err(|_elapsed| {
            zbus::Error::Failure("session bus identity probe timed out".to_string())
        })?
        .map_err(zbus::Error::from)?;
    let unique_name = connection
        .unique_name()
        .ok_or_else(|| zbus::Error::Failure("session bus has no unique name".to_string()))?;
    let identity = SessionBusIdentity {
        bus_id: bus_id.to_string(),
        unique_name: unique_name.to_string(),
        runtime_dir: std::env::var("XDG_RUNTIME_DIR").unwrap_or_default(),
    };

    info!(
        bus_id = %identity.bus_id,
        unique_name = %identity.unique_name,
        runtime_dir = %identity.runtime_dir,
        component,
        "connected to session bus"
    );
    Ok(identity)
}
