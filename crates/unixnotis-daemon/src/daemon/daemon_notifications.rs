//! D-Bus server for org.freedesktop.Notifications
//!
//! The interface methods live here while parsing/sanitizing helpers are split into
//! focused submodules under `daemon_notifications/`

use std::collections::HashMap;
use std::sync::Arc;

use tracing::debug;
use unixnotis_core::{CloseReason, CONTROL_OBJECT_PATH};
use zbus::message::Header;
use zbus::zvariant::OwnedValue;
use zbus::{interface, SignalContext};

use crate::expire::ExpirationScheduler;

use super::{
    to_fdo_error, ControlServer, DaemonState, NotificationSignalMode, NOTIFICATIONS_OBJECT_PATH,
};

// Split hard limits into a dedicated file so security bounds are easy to review
#[path = "daemon_notifications/limits.rs"]
mod limits;
// Split payload construction into a dedicated file to keep interface code compact
#[path = "daemon_notifications/payload.rs"]
mod payload;
// Split sender metadata helpers so ownership logic is reused consistently
#[path = "daemon_notifications/sender.rs"]
mod sender;

use payload::{build_notification, resolve_expiration, NotificationInput};
use sender::{app_name_matches_sender, resolve_sender_metadata};

/// D-Bus server for org.freedesktop.Notifications
pub struct NotificationServer {
    // Shared daemon state for store access, sounds, and signal emission
    state: Arc<DaemonState>,
    // Scheduler handles expiration deadlines without blocking D-Bus handlers
    scheduler: ExpirationScheduler,
}

impl NotificationServer {
    pub fn new(state: Arc<DaemonState>, scheduler: ExpirationScheduler) -> Self {
        // Keep constructor minimal and explicit
        Self { state, scheduler }
    }
}

#[interface(name = "org.freedesktop.Notifications")]
impl NotificationServer {
    async fn get_capabilities(&self) -> Vec<String> {
        // Capabilities are static except for optional sound support
        let mut caps = vec![
            "actions".to_string(),
            "body".to_string(),
            "body-markup".to_string(),
            "icon-static".to_string(),
        ];
        if self.state.sound.supports_sound() {
            caps.push("sound".to_string());
        }
        caps
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
        // Debug logging is guarded so normal operation keeps log volume small
        if tracing::enabled!(tracing::Level::DEBUG) {
            let summary_snip = unixnotis_core::util::log_snippet(&summary);
            debug!(
                app = %app_name,
                summary = %summary_snip,
                summary_len = summary.len(),
                body_len = body.len(),
                replaces_id,
                expire_timeout,
                "received notification"
            );
            if unixnotis_core::util::diagnostic_mode() {
                let body_snip = unixnotis_core::util::log_snippet(&body);
                debug!(body = %body_snip, "notification body snippet");
            }
        }

        // Sender metadata helps with ownership checks and diagnostics
        let sender = resolve_sender_metadata(self.state.connection(), &header).await;
        if sender
            .sender_executable
            .as_deref()
            .is_some_and(|exe| !app_name_matches_sender(&app_name, exe))
        {
            debug!(
                app_name = %app_name,
                sender = sender.sender_name.as_deref().unwrap_or("unknown"),
                sender_executable = sender.sender_executable.as_deref().unwrap_or("unknown"),
                "notification app_name does not match sender executable"
            );
        }

        // Build a safe notification record from untrusted wire data
        let notification = build_notification(NotificationInput {
            app_name,
            app_icon,
            summary,
            body,
            actions,
            hints,
            sender,
            expire_timeout,
        });

        // Store mutation and expiration scheduling happen under one lock scope
        let (outcome, expiration) = {
            let mut store = self.state.store.lock().await;
            let outcome = store.insert(notification, replaces_id);
            let expiration = if outcome.dropped {
                None
            } else {
                // Resolve timeout after insertion so rule-mapped fields are already final
                let expiration = resolve_expiration(store.config(), &outcome.notification);
                store.set_expiration(outcome.notification.id, expiration);
                expiration
            };
            (outcome, expiration)
        };

        // Drop-all inhibition path intentionally skips signals, sound, and scheduler
        if outcome.dropped {
            debug!(
                id = outcome.notification.id,
                app = %outcome.notification.app_name,
                "notification dropped due to active inhibitor"
            );
            return Ok(outcome.notification.id);
        }

        self.scheduler
            .schedule(outcome.notification.id, expiration)
            .await;

        // Sound is best-effort and decided by rules and per-notification hints
        self.state
            .sound
            .play_from_hints(&outcome.notification.hints, outcome.allow_sound);

        let control_ctx = SignalContext::new(self.state.connection(), CONTROL_OBJECT_PATH)
            .map_err(to_fdo_error)?;
        match self
            .state
            .notification_signal_mode(outcome.notification.sender_name.as_deref())
        {
            NotificationSignalMode::Direct => {
                if outcome.replaced {
                    // Only the id crosses the broadcast signal
                    // Trusted UIs fetch the live payload through the authorized control API
                    ControlServer::notification_updated(
                        &control_ctx,
                        outcome.notification.id,
                        outcome.show_popup,
                    )
                    .await
                    .map_err(to_fdo_error)?;
                } else {
                    // New notification broadcasts only the id for the same confidentiality reason
                    ControlServer::notification_added(
                        &control_ctx,
                        outcome.notification.id,
                        outcome.show_popup,
                    )
                    .await
                    .map_err(to_fdo_error)?;
                }
            }
            NotificationSignalMode::SnapshotOnly => {
                debug!(
                    id = outcome.notification.id,
                    sender = outcome.notification.sender_name.as_deref().unwrap_or("unknown"),
                    "notification burst detected; using snapshot invalidation instead of per-row signal"
                );
                self.state
                    .emit_snapshot_invalidated()
                    .await
                    .map_err(to_fdo_error)?;
            }
            NotificationSignalMode::Suppress => {}
        }

        // Evicted items are announced so UIs can remove stale rows
        self.handle_evicted(outcome.evicted).await?;
        self.state
            .emit_state_changed()
            .await
            .map_err(to_fdo_error)?;

        Ok(outcome.notification.id)
    }

    async fn handle_evicted(&self, evicted: Vec<u32>) -> zbus::fdo::Result<()> {
        if evicted.is_empty() {
            // Fast path avoids context allocation when no eviction happened
            return Ok(());
        }

        let notif_ctx = SignalContext::new(self.state.connection(), NOTIFICATIONS_OBJECT_PATH)
            .map_err(to_fdo_error)?;
        let control_ctx = SignalContext::new(self.state.connection(), CONTROL_OBJECT_PATH)
            .map_err(to_fdo_error)?;

        for id in evicted {
            // Emit both freedesktop and control close signals for consistent subscribers
            NotificationServer::notification_closed(&notif_ctx, id, CloseReason::Undefined as u32)
                .await
                .map_err(to_fdo_error)?;
            ControlServer::notification_closed(&control_ctx, id, CloseReason::Undefined)
                .await
                .map_err(to_fdo_error)?;
        }
        Ok(())
    }

    async fn close_notification(
        &self,
        id: u32,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<()> {
        debug!(id, "close notification requested");

        // Close requests are ownership checked and become no-op when unauthorized
        let sender = resolve_sender_metadata(self.state.connection(), &header).await;
        let Some(sender_name) = sender.sender_name.as_deref() else {
            return Ok(());
        };

        let owned = {
            let store = self.state.store.lock().await;
            // Ownership check allows reconnect-safe close by same sender pid
            store.is_notification_owned_by(
                id,
                sender_name,
                sender.sender_pid,
                sender.sender_start_time,
            )
        };
        if !owned {
            debug!(
                id,
                sender = sender_name,
                sender_pid = sender.sender_pid,
                "ignoring close for unowned notification"
            );
            return Ok(());
        }

        self.state
            .close_notification(id, CloseReason::ClosedByCall)
            .await
            .map_err(to_fdo_error)
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
