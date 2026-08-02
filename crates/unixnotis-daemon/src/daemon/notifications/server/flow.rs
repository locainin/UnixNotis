use std::collections::HashMap;
use tracing::{debug, warn};
use unixnotis_core::{ImageData, Notification, NotificationKey};
use zbus::message::Header;
use zbus::zvariant::OwnedValue;

use crate::daemon::notifications::identity::{
    resolve_attribution_owned, resolve_attribution_with_deadline, SenderMetadata,
};
use crate::daemon::notifications::identity::{
    resolve_sender_metadata, SenderMetadataStatus, SENDER_CREDENTIAL_TIMEOUT,
};
use crate::daemon::notifications::ingress::payload::{
    build_notification, materialize_sender_visual, owned_to_string, resolve_expiration,
    sender_visual_role, NotificationInput, SenderVisualRole, CONVERSATION_AVATAR_TIMEOUT,
};
use crate::daemon::{to_fdo_error, NotificationSignalMode};
use crate::store::InsertOutcome;

use super::avatar::run_avatar_worker;
use super::wire_hints::WireHints;
use super::NotificationServer;

struct StoredNotification {
    outcome: InsertOutcome,
}

struct WireNotification {
    app_name: String,
    app_icon: String,
    summary: String,
    body: String,
    actions: Vec<String>,
    hints: HashMap<String, OwnedValue>,
    image_data: Option<ImageData>,
    image_path: Option<String>,
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
        hints: WireHints,
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
        let (hints, image_data, image_path) = hints.into_parts();
        let notification = self
            .notification_from_wire(
                WireNotification {
                    app_name,
                    app_icon,
                    summary,
                    body,
                    actions,
                    hints,
                    image_data,
                    image_path,
                    expire_timeout,
                },
                header,
            )
            .await;
        let stored = self.store_notification(notification, replaces_id).await;
        self.finish_notification_change(stored.outcome).await
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
        let sender = if let Ok(sender) = tokio::time::timeout(
            SENDER_CREDENTIAL_TIMEOUT,
            resolve_sender_metadata(
                &self.state.sender_metadata_cache,
                self.state.connection(),
                header,
            ),
        )
        .await
        {
            sender
        } else {
            warn!("notification sender credentials timed out and failed closed");
            SenderMetadata {
                status: SenderMetadataStatus::CredentialLookupTimedOut,
                ..SenderMetadata::default()
            }
        };
        let desktop_entry = input.hints.get("desktop-entry").and_then(owned_to_string);
        let desktop_identity_index = self.state.desktop_identity_index.load_full();
        // This is the only attribution deadline, including package enrichment
        let resolution = resolve_attribution_with_deadline(
            input.app_name.clone(),
            desktop_entry.clone(),
            &sender,
            resolve_attribution_owned(
                input.app_name.clone(),
                desktop_entry.clone(),
                sender.clone(),
                std::sync::Arc::clone(&desktop_identity_index),
            ),
        )
        .await;
        let sender_visual_role = sender_visual_role(
            &resolution.attribution,
            &desktop_identity_index,
            &input.hints,
            &input.actions,
            &input.app_icon,
        );
        let sender_visual =
            materialize_sender_visual_for_role(sender_visual_role, input.app_icon.clone()).await;
        let materialized_content =
            materialize_content_visual(&resolution.attribution, input.image_path.as_deref()).await;
        if matches!(
            resolution.attribution.status,
            unixnotis_core::AttributionStatus::Conflict
        ) {
            debug!(
                app_name = %input.app_name,
                sender = sender.sender_name.as_deref().unwrap_or("unknown"),
                sender_executable = sender.sender_executable.as_deref().unwrap_or("unknown"),
                detail = %resolution.attribution.diagnostic_detail,
                "notification application claim conflicts with sender evidence"
            );
        }
        debug!(
            claim = %resolution.diagnostics.claimed_name,
            desktop_entry = %resolution.diagnostics.claimed_desktop_entry,
            sender_executable = %resolution.diagnostics.sender_executable,
            matched_desktop_id = %resolution.diagnostics.matched_desktop_id,
            record_origin = ?resolution.diagnostics.record_trust,
            launch_authority = ?resolution.diagnostics.launch_authority,
            cmdline_quality = ?resolution.diagnostics.command_line_quality,
            verification = ?resolution.diagnostics.verification,
            reason = %resolution.diagnostics.reason,
            "notification attribution decided"
        );

        // Build a safe notification record from untrusted wire data
        build_notification(NotificationInput {
            app_name: input.app_name,
            app_icon: input.app_icon,
            summary: input.summary,
            body: input.body,
            actions: input.actions,
            hints: input.hints,
            image_data: input.image_data.or(materialized_content),
            sender_visual,
            sender_visual_role,
            sender,
            attribution: resolution.attribution,
            attribution_diagnostics: resolution.diagnostics,
            inline_reply_policy: resolution.inline_reply_policy,
            expire_timeout: input.expire_timeout,
        })
    }

    async fn store_notification(
        &self,
        notification: Notification,
        replaces_id: u32,
    ) -> StoredNotification {
        // Store mutation and scheduler delivery share one serialized lock scope
        let outcome = {
            let mut store = self.state.store.lock().await;
            // Sample renderer health immediately before the serialized commit
            let ui_health = self.state.ui_health();
            let outcome = store.insert_with_ui_health(notification, replaces_id, &ui_health);
            if !outcome.dropped {
                // Resolve timeout after insertion so rule-mapped fields are already final
                let expiration = resolve_expiration(store.config(), &outcome.notification);
                store.set_expiration(&outcome.notification, expiration);
                // Unbounded send is synchronous, so commit order is preserved without an await
                self.scheduler.schedule(
                    outcome.notification.id,
                    outcome.notification.generation,
                    expiration,
                );
            }
            // Eviction cancellation is committed in the same order as the insertion
            for key in &outcome.evicted {
                self.scheduler.schedule(key.id, key.generation, None);
            }
            outcome
        };
        StoredNotification { outcome }
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

    fn play_sound(&self, outcome: &InsertOutcome) {
        // Sound is best-effort and decided by rules and per-notification hints
        self.state
            .sound
            .play_from_hints(&outcome.notification.hints, outcome.allow_sound);
    }

    async fn emit_notification_change(&self, outcome: &InsertOutcome) -> zbus::fdo::Result<()> {
        let mode = self
            .state
            .notification_signal_mode(outcome.notification.sender_name.as_deref());
        if mode == NotificationSignalMode::SnapshotOnly {
            debug!(
                id = outcome.notification.id,
                sender = outcome.notification.sender_name.as_deref().unwrap_or("unknown"),
                "notification burst detected; using snapshot invalidation instead of per-row signal"
            );
        }
        self.state
            .publish_notification_change(mode, outcome.notification.key(), outcome.replaced)
            .await
            .map_err(to_fdo_error)
    }

    async fn finish_notification_change(&self, outcome: InsertOutcome) -> zbus::fdo::Result<u32> {
        if let Some(id) = Self::handle_dropped_notification(&outcome) {
            return Ok(id);
        }

        self.play_sound(&outcome);
        debug!(
            id = outcome.notification.id,
            decision = ?outcome.popup_admission,
            "notification popup admission decided"
        );
        if outcome.popup_admission.should_show() && self.state.should_warn_popups_unready() {
            warn!(
                id = outcome.notification.id,
                "popup admitted while popup renderer is not ready"
            );
        }
        let id = outcome.notification.id;
        if let Err(error) = self.emit_notification_change(&outcome).await {
            warn!(?error, id, "notification committed but live fanout failed");
            self.state.store.lock().await.record_popup_delivery_stage(
                outcome.notification.key(),
                unixnotis_core::PopupDeliveryStage::FanoutFailed,
            );
            // Snapshot invalidation gives connected clients one best-effort recovery route
            let _ = self.state.publish_snapshot_invalidated().await;
        }
        // Evicted items are announced so UIs can remove stale rows
        if let Err(error) = self.handle_evicted(outcome.evicted).await {
            warn!(
                ?error,
                id, "notification committed but eviction fanout failed"
            );
        }
        if let Err(error) = self.state.publish_state_changed().await {
            warn!(?error, id, "notification committed but state fanout failed");
        }

        Ok(id)
    }

    async fn handle_evicted(&self, evicted: Vec<NotificationKey>) -> zbus::fdo::Result<()> {
        if evicted.is_empty() {
            // Fast path avoids context allocation when no eviction happened
            return Ok(());
        }
        self.state
            .publish_evicted_notifications(&evicted)
            .await
            .map_err(to_fdo_error)
    }
}

async fn materialize_sender_visual_for_role(
    role: SenderVisualRole,
    app_icon: String,
) -> Option<ImageData> {
    if matches!(role, SenderVisualRole::None) {
        return None;
    }
    run_avatar_worker(
        move || materialize_sender_visual(&app_icon, 64),
        CONVERSATION_AVATAR_TIMEOUT,
    )
    .await
    .flatten()
}

async fn materialize_content_visual(
    attribution: &unixnotis_core::NotificationAttribution,
    image_path: Option<&str>,
) -> Option<ImageData> {
    if !crate::daemon::notifications::ingress::payload::may_read_sender_host_visual(attribution) {
        return None;
    }
    let path = image_path
        .filter(|path| !path.trim().is_empty())
        .map(str::to_owned)?;
    run_avatar_worker(
        move || materialize_sender_visual(&path, 512),
        CONVERSATION_AVATAR_TIMEOUT,
    )
    .await
    .flatten()
}

#[cfg(test)]
#[path = "tests/flow.rs"]
mod tests;
