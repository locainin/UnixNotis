//! Notification D-Bus interface implementation

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::Semaphore;
use zbus::message::Header;
use zbus::zvariant::OwnedValue;
use zbus::{interface, SignalContext};

use crate::expire::ExpirationScheduler;

use super::capabilities::notification_capabilities;
use crate::daemon::notifications::quota::NotificationQuota;
use crate::daemon::DaemonState;

const MAX_CONCURRENT_NOTIFY_HANDLERS: usize = 8;

/// D-Bus server for org.freedesktop.Notifications
pub struct NotificationServer {
    // Shared daemon state for store access, sounds, and signal emission
    pub(super) state: Arc<DaemonState>,
    // Scheduler handles expiration deadlines without blocking D-Bus handlers
    pub(super) scheduler: ExpirationScheduler,
    // Shared token buckets reject sustained sender and process-wide floods
    quota: NotificationQuota,
    // Expensive sender and payload work has a fixed concurrency ceiling
    notify_slots: Semaphore,
}

impl NotificationServer {
    pub fn new(state: Arc<DaemonState>, scheduler: ExpirationScheduler) -> Self {
        // Keep constructor minimal and explicit
        Self {
            state,
            scheduler,
            quota: NotificationQuota::new(),
            notify_slots: Semaphore::const_new(MAX_CONCURRENT_NOTIFY_HANDLERS),
        }
    }
}

#[interface(name = "org.freedesktop.Notifications")]
impl NotificationServer {
    pub(super) async fn get_capabilities(&self) -> Vec<String> {
        // Advertise sound support only when the configured backend can deliver it
        notification_capabilities(self.state.sound.supports_sound())
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the freedesktop notification D-Bus signature fixes this argument list"
    )]
    pub(super) async fn notify(
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
        let sender = header.sender().map(zbus::names::UniqueName::as_str);
        if !self.quota.admit(sender, Instant::now()) {
            return Err(zbus::fdo::Error::LimitsExceeded(
                "notification ingress quota exceeded".to_string(),
            ));
        }
        let _slot = self.notify_slots.try_acquire().map_err(|_error| {
            zbus::fdo::Error::LimitsExceeded(
                "too many concurrent notification requests".to_string(),
            )
        })?;
        // The interface adapter forwards the authenticated header with the exact wire payload
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

    pub(super) async fn close_notification(
        &self,
        id: u32,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<()> {
        // Ownership checks remain in the shared close path used by all D-Bus callers
        self.close_notification_if_owned(id, &header).await
    }

    pub(super) async fn get_server_information(&self) -> (String, String, String, String) {
        // Keep server information stable for freedesktop client compatibility
        (
            "UnixNotis".to_string(),
            "UnixNotis".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
            "1.2".to_string(),
        )
    }

    #[zbus(signal)]
    // Signal declarations define the freedesktop wire contract and are emitted elsewhere
    pub(crate) async fn notification_closed(
        ctx: &SignalContext<'_>,
        id: u32,
        reason: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    // Action keys are passed through unchanged so clients can match their registered action
    pub(crate) async fn action_invoked(
        ctx: &SignalContext<'_>,
        id: u32,
        action_key: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    // KDE-compatible senders receive the entered text through this extension signal
    pub(crate) async fn notification_replied(
        ctx: &SignalContext<'_>,
        id: u32,
        reply_text: &str,
    ) -> zbus::Result<()>;
}
