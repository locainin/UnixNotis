use std::collections::HashMap;
use std::time::Instant;

use tracing::debug;
use unixnotis_core::{CloseReason, Notification, CONTROL_OBJECT_PATH};
use zbus::message::Header;
use zbus::zvariant::OwnedValue;
use zbus::SignalContext;

use crate::daemon::notifications::payload::{
    build_notification, resolve_expiration, NotificationInput,
};
use crate::daemon::notifications::sender::{app_name_matches_sender, resolve_sender_metadata};
use crate::daemon::{
    to_fdo_error, ControlServer, NotificationSignalMode, NOTIFICATIONS_OBJECT_PATH,
};
use crate::store::InsertOutcome;

use super::NotificationServer;

struct StoredNotification {
    outcome: InsertOutcome,
    expiration: Option<Instant>,
}

struct WireNotification {
    app_name: String,
    app_icon: String,
    summary: String,
    body: String,
    actions: Vec<String>,
    hints: HashMap<String, OwnedValue>,
    expire_timeout: i32,
}

impl NotificationServer {
    #[expect(
        clippy::too_many_arguments,
        reason = "the freedesktop notification method defines this wire-level argument list"
    )]
    pub(super) async fn ingest_notify(
        &self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        header: &Header<'_>,
        expire_timeout: i32,
    ) -> zbus::fdo::Result<u32> {
        let _ = Self::log_received_notification(
            &app_name,
            &summary,
            &body,
            replaces_id,
            expire_timeout,
        );
        let notification = self
            .notification_from_wire(
                WireNotification {
                    app_name,
                    app_icon,
                    summary,
                    body,
                    actions,
                    hints,
                    expire_timeout,
                },
                header,
            )
            .await;
        let stored = self.store_notification(notification, replaces_id).await;
        self.finish_notification_change(stored.outcome, stored.expiration)
            .await
    }

    fn log_received_notification(
        app_name: &str,
        summary: &str,
        body: &str,
        replaces_id: u32,
        expire_timeout: i32,
    ) -> bool {
        // Debug logging is guarded so normal operation keeps log volume small
        if !tracing::enabled!(tracing::Level::DEBUG) {
            return false;
        }
        let summary_snip = unixnotis_core::util::log_snippet(summary);
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
            let body_snip = unixnotis_core::util::log_snippet(body);
            debug!(body = %body_snip, "notification body snippet");
        }
        true
    }

    async fn notification_from_wire(
        &self,
        input: WireNotification,
        header: &Header<'_>,
    ) -> Notification {
        // Sender metadata helps with ownership checks and diagnostics
        let sender = resolve_sender_metadata(self.state.connection(), header).await;
        if sender_app_name_mismatch(&input.app_name, sender.sender_executable.as_deref()) {
            debug!(
                app_name = %input.app_name,
                sender = sender.sender_name.as_deref().unwrap_or("unknown"),
                sender_executable = sender.sender_executable.as_deref().unwrap_or("unknown"),
                "notification app_name does not match sender executable"
            );
        }

        // Build a safe notification record from untrusted wire data
        build_notification(NotificationInput {
            app_name: input.app_name,
            app_icon: input.app_icon,
            summary: input.summary,
            body: input.body,
            actions: input.actions,
            hints: input.hints,
            sender,
            expire_timeout: input.expire_timeout,
        })
    }

    async fn store_notification(
        &self,
        notification: Notification,
        replaces_id: u32,
    ) -> StoredNotification {
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
        StoredNotification {
            outcome,
            expiration,
        }
    }

    fn handle_dropped_notification(outcome: &InsertOutcome) -> Option<u32> {
        if !outcome.dropped {
            return None;
        }
        debug!(
            id = outcome.notification.id,
            app = %outcome.notification.app_name,
            "notification dropped due to active inhibitor"
        );
        Some(outcome.notification.id)
    }

    fn schedule_and_play(&self, outcome: &InsertOutcome, expiration: Option<Instant>) {
        self.scheduler.schedule(outcome.notification.id, expiration);
        // Sound is best-effort and decided by rules and per-notification hints
        self.state
            .sound
            .play_from_hints(&outcome.notification.hints, outcome.allow_sound);
    }

    async fn emit_notification_change(&self, outcome: &InsertOutcome) -> zbus::fdo::Result<()> {
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
        Ok(())
    }

    async fn finish_notification_change(
        &self,
        outcome: InsertOutcome,
        expiration: Option<Instant>,
    ) -> zbus::fdo::Result<u32> {
        if let Some(id) = Self::handle_dropped_notification(&outcome) {
            return Ok(id);
        }

        self.schedule_and_play(&outcome, expiration);
        self.emit_notification_change(&outcome).await?;
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
        self.state.cancel_expirations(&evicted);

        let notif_ctx = SignalContext::new(self.state.connection(), NOTIFICATIONS_OBJECT_PATH)
            .map_err(to_fdo_error)?;
        let control_ctx = SignalContext::new(self.state.connection(), CONTROL_OBJECT_PATH)
            .map_err(to_fdo_error)?;

        for id in evicted {
            // Emit both freedesktop and control close signals for consistent subscribers
            Self::notification_closed(&notif_ctx, id, CloseReason::Undefined as u32)
                .await
                .map_err(to_fdo_error)?;
            ControlServer::notification_closed(&control_ctx, id, CloseReason::Undefined)
                .await
                .map_err(to_fdo_error)?;
        }
        Ok(())
    }
}

fn sender_app_name_mismatch(app_name: &str, sender_executable: Option<&str>) -> bool {
    sender_executable.is_some_and(|exe| !app_name_matches_sender(app_name, exe))
}

#[cfg(test)]
#[path = "../tests/flow.rs"]
mod tests;
