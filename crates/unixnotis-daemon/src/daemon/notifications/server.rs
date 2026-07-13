//! Notification D-Bus interface implementation

use std::collections::HashMap;
use std::sync::Arc;

use zbus::message::Header;
use zbus::zvariant::OwnedValue;
use zbus::{interface, SignalContext};

use crate::expire::ExpirationScheduler;

use crate::daemon::DaemonState;
use capabilities::notification_capabilities;

mod capabilities;
mod close;
mod flow;
#[cfg(test)]
#[path = "tests/server.rs"]
mod tests;

/// D-Bus server for org.freedesktop.Notifications
pub struct NotificationServer {
    // Shared daemon state for store access, sounds, and signal emission
    state: Arc<DaemonState>,
    // Scheduler handles expiration deadlines without blocking D-Bus handlers
    scheduler: ExpirationScheduler,
}

impl NotificationServer {
    pub const fn new(state: Arc<DaemonState>, scheduler: ExpirationScheduler) -> Self {
        // Keep constructor minimal and explicit
        Self { state, scheduler }
    }
}

#[interface(name = "org.freedesktop.Notifications")]
impl NotificationServer {
    async fn get_capabilities(&self) -> Vec<String> {
        notification_capabilities(self.state.sound.supports_sound())
    }

    #[allow(clippy::too_many_arguments)]
    async fn notify(
        &self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        #[zbus(header)] header: Header<'_>,
        expire_timeout: i32,
    ) -> zbus::fdo::Result<u32> {
        self.ingest_notify(
            app_name,
            replaces_id,
            app_icon,
            summary,
            body,
            actions,
            hints,
            &header,
            expire_timeout,
        )
        .await
    }

    async fn close_notification(
        &self,
        id: u32,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<()> {
        self.close_notification_if_owned(id, &header).await
    }

    async fn get_server_information(&self) -> (String, String, String, String) {
        // Keep server information stable for freedesktop client compatibility
        (
            "UnixNotis".to_string(),
            "UnixNotis".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
            "1.2".to_string(),
        )
    }

    #[zbus(signal)]
    pub(crate) async fn notification_closed(
        ctx: &SignalContext<'_>,
        id: u32,
        reason: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub(crate) async fn action_invoked(
        ctx: &SignalContext<'_>,
        id: u32,
        action_key: &str,
    ) -> zbus::Result<()>;
}
