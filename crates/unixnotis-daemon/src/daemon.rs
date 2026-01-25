//! D-Bus server implementation and daemon state coordination.
//!
//! The notification and control interfaces are split into submodules to keep
//! responsibilities clear and files smaller.

use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::info;
use unixnotis_core::{CloseReason, Config, CONTROL_BUS_NAME, CONTROL_OBJECT_PATH};
use zbus::fdo::{RequestNameFlags, RequestNameReply};
use zbus::{Connection, SignalContext};

use crate::sound::SoundSettings;
use crate::store::NotificationStore;

#[path = "daemon/daemon_control.rs"]
mod daemon_control;
#[path = "daemon/daemon_notifications.rs"]
mod daemon_notifications;

pub use daemon_control::{spawn_inhibitor_owner_watch, ControlServer};
pub use daemon_notifications::NotificationServer;

pub(crate) const NOTIFICATIONS_OBJECT_PATH: &str = "/org/freedesktop/Notifications";

/// Shared daemon state guarded behind an async mutex.
pub struct DaemonState {
    pub store: Mutex<NotificationStore>,
    /// Immutable sound settings resolved at startup.
    pub sound: SoundSettings,
    connection: Connection,
}

impl DaemonState {
    pub fn new(connection: Connection, config: Config, sound: SoundSettings) -> Arc<Self> {
        let store = NotificationStore::new(config);
        Arc::new(Self {
            store: Mutex::new(store),
            sound,
            connection,
        })
    }

    pub async fn close_notification(&self, id: u32, reason: CloseReason) -> zbus::Result<()> {
        let removed = {
            let mut store = self.store.lock().await;
            store.close(id)
        };
        if removed.is_none() {
            return Ok(());
        }

        let notif_ctx = SignalContext::new(&self.connection, NOTIFICATIONS_OBJECT_PATH)?;
        NotificationServer::notification_closed(&notif_ctx, id, reason as u32).await?;

        let control_ctx = SignalContext::new(&self.connection, CONTROL_OBJECT_PATH)?;
        ControlServer::notification_closed(&control_ctx, id, reason).await?;
        self.emit_state_changed().await?;

        Ok(())
    }

    pub async fn dismiss_from_panel(&self, id: u32) -> zbus::Result<()> {
        let outcome = {
            let mut store = self.store.lock().await;
            store.dismiss_from_panel(id)
        };

        if !outcome.removed_any() {
            return Ok(());
        }

        if outcome.removed_active {
            let notif_ctx = SignalContext::new(&self.connection, NOTIFICATIONS_OBJECT_PATH)?;
            NotificationServer::notification_closed(
                &notif_ctx,
                id,
                CloseReason::DismissedByUser as u32,
            )
            .await?;
        }

        let control_ctx = SignalContext::new(&self.connection, CONTROL_OBJECT_PATH)?;
        ControlServer::notification_closed(&control_ctx, id, CloseReason::DismissedByUser).await?;
        self.emit_state_changed().await?;

        Ok(())
    }

    async fn emit_state_changed(&self) -> zbus::Result<()> {
        let state = {
            let store = self.store.lock().await;
            let history_count = store.history_len() as u32;
            unixnotis_core::ControlState {
                dnd_enabled: store.dnd_enabled(),
                history_count,
                inhibited: store.inhibited(),
                inhibitor_count: store.inhibitor_count(),
            }
        };
        let control_ctx = SignalContext::new(&self.connection, CONTROL_OBJECT_PATH)?;
        ControlServer::state_changed(&control_ctx, state).await
    }

    pub(crate) fn connection(&self) -> &Connection {
        &self.connection
    }
}

pub async fn request_well_known_name(
    connection: &Connection,
    replace_existing: bool,
) -> zbus::Result<RequestNameReply> {
    let flags = if replace_existing {
        zbus::fdo::RequestNameFlags::ReplaceExisting | zbus::fdo::RequestNameFlags::AllowReplacement
    } else {
        // Avoid being replaceable in non-trial mode to prevent silent takeovers.
        zbus::fdo::RequestNameFlags::DoNotQueue.into()
    };
    connection
        .request_name_with_flags("org.freedesktop.Notifications", flags)
        .await
}

pub async fn request_control_name(connection: &Connection) -> zbus::Result<RequestNameReply> {
    let flags = RequestNameFlags::DoNotQueue;
    connection
        .request_name_with_flags(CONTROL_BUS_NAME, flags.into())
        .await
}

pub fn log_name_reply(reply: &RequestNameReply) {
    match reply {
        RequestNameReply::PrimaryOwner => {
            info!("acquired org.freedesktop.Notifications");
        }
        RequestNameReply::InQueue => {
            info!("queued for org.freedesktop.Notifications");
        }
        RequestNameReply::AlreadyOwner => {
            info!("already owns org.freedesktop.Notifications");
        }
        RequestNameReply::Exists => {
            info!("org.freedesktop.Notifications is already owned");
        }
    }
}

pub(crate) fn to_fdo_error(err: zbus::Error) -> zbus::fdo::Error {
    zbus::fdo::Error::Failed(err.to_string())
}
