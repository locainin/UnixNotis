use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
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
use crate::daemon::notifications::ingress::metrics::RejectedRequest;
use crate::daemon::notifications::ingress::payload::{
    build_notification, materialize_sender_visual, may_materialize_content_image, owned_to_string,
    sender_visual_role, NotificationInput, SenderVisualRole, CONVERSATION_AVATAR_TIMEOUT,
    MAX_STORED_AVATAR_DIMENSION, MAX_STORED_CONTENT_DIMENSION,
};
use crate::daemon::{to_fdo_error, NotificationSignalMode};
use crate::store::{CommitDisposition, InsertOutcome, SuppressedNotification};

use super::avatar::run_avatar_worker;
use super::reply_lifecycle::NotifyCompletion;
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
    wire_image_data: Option<super::wire_hints::WireImageData>,
    image_path: Option<String>,
    expire_timeout: i32,
}

impl NotificationServer {
    #[expect(
        clippy::too_many_arguments,
        reason = "the freedesktop notification method defines this wire-level argument list"
    )]
    pub(super) async fn ingest_notify_deferred(
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
    ) -> zbus::fdo::Result<NotifyCompletion> {
        let _ = Self::log_received_notification(
            &app_name,
            &summary,
            &body,
            replaces_id,
            expire_timeout,
        );
        let sender = self.resolve_sender(header).await;
        if !self
            .notify_quota
            .admit_principal(super::quota_principal(&sender), Instant::now())
        {
            let rejected = self
                .ingress_metrics
                .record_rejection(RejectedRequest::NotifyQuota);
            debug!(rejected, "notification request rejected by principal quota");
            return Err(zbus::fdo::Error::LimitsExceeded(
                "notification ingress quota exceeded".to_string(),
            ));
        }
        let (hints, wire_image_data, image_path) = hints.into_parts();
        let notification = self
            .notification_from_wire(
                WireNotification {
                    app_name,
                    app_icon,
                    summary,
                    body,
                    actions,
                    hints,
                    wire_image_data,
                    image_path,
                    expire_timeout,
                },
                sender,
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

    async fn resolve_sender(&self, header: &Header<'_>) -> SenderMetadata {
        // Sender metadata helps with ownership checks and diagnostics
        if let Ok(sender) = tokio::time::timeout(
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
        }
    }

    async fn notification_from_wire(
        &self,
        input: WireNotification,
        sender: SenderMetadata,
    ) -> Notification {
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
        let (image_data, wire_sender_visual) = normalize_wire_image_for_role(
            sender_visual_role,
            input.wire_image_data,
            materialized_content,
        );
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
            image_data,
            sender_visual_data: wire_sender_visual,
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
            if let CommitDisposition::Active(notification) = &outcome.disposition {
                // The store resolved both clocks after applying rules and committing the generation
                let expiration = outcome.expiration;
                store.set_expiration(notification, expiration);
                // Unbounded send is synchronous, so commit order is preserved without an await
                self.scheduler
                    .schedule(notification.id, notification.generation, expiration);
            }
            // Eviction cancellation is committed in the same order as the insertion
            for key in &outcome.evicted {
                self.scheduler.schedule(key.id, key.generation, None);
            }
            outcome
        };
        StoredNotification { outcome }
    }

    fn suppressed_notification(outcome: &InsertOutcome) -> Option<SuppressedNotification> {
        let suppressed = outcome.suppressed()?;
        debug!(
            id = suppressed.id,
            generation = suppressed.generation,
            owner_pid = suppressed.owner.map(|owner| owner.pid),
            "notification content dropped due to active inhibitor"
        );
        Some(suppressed)
    }

    fn play_sound(&self, notification: &Notification, allow_sound: bool) -> bool {
        // Sound is best-effort and decided by rules and per-notification hints
        self.state
            .sound
            .play_from_hints(&notification.hints, allow_sound)
    }

    async fn emit_notification_change(
        &self,
        notification: &Notification,
        replaced: bool,
    ) -> zbus::fdo::Result<()> {
        let mode = self
            .state
            .notification_signal_mode(notification.sender_name.as_deref());
        if mode == NotificationSignalMode::SnapshotOnly {
            debug!(
                id = notification.id,
                sender = notification.sender_name.as_deref().unwrap_or("unknown"),
                "notification burst detected; using snapshot invalidation instead of per-row signal"
            );
        }
        self.state
            .publish_notification_change(mode, notification.key(), replaced)
            .await
            .map_err(to_fdo_error)
    }

    async fn finish_notification_change(
        &self,
        outcome: InsertOutcome,
    ) -> zbus::fdo::Result<NotifyCompletion> {
        let notification = match &outcome.disposition {
            CommitDisposition::Active(notification) => Arc::clone(notification),
            CommitDisposition::SuppressedDropAll(suppressed) => {
                let suppressed = *suppressed;
                let _ = Self::suppressed_notification(&outcome);
                return Ok(NotifyCompletion {
                    id: suppressed.id,
                    suppressed: Some(suppressed),
                });
            }
        };
        let _sound_accepted = self.play_sound(&notification, outcome.allow_sound);
        debug!(
            id = notification.id,
            decision = ?outcome.popup_admission,
            "notification popup admission decided"
        );
        if outcome.popup_admission.should_show() && self.state.should_warn_popups_unready() {
            warn!(
                id = notification.id,
                "popup admitted while popup renderer is not ready"
            );
        }
        let id = notification.id;
        let key = notification.key();
        if let Err(error) = self
            .emit_notification_change(&notification, outcome.replaced)
            .await
        {
            warn!(?error, id, "notification committed but live fanout failed");
            self.state
                .store
                .lock()
                .await
                .record_popup_delivery_stage(key, unixnotis_core::PopupDeliveryStage::FanoutFailed);
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

        Ok(NotifyCompletion {
            id,
            suppressed: None,
        })
    }

    pub(super) async fn publish_suppressed_close(&self, suppressed: SuppressedNotification) {
        let key = NotificationKey {
            id: suppressed.id,
            generation: suppressed.generation,
        };
        if let Err(error) = self
            .state
            .publish_notification_closed(key, unixnotis_core::CloseReason::Undefined)
            .await
        {
            warn!(
                ?error,
                id = suppressed.id,
                generation = suppressed.generation,
                "suppressed notification close fanout failed"
            );
        }
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

fn normalize_wire_image_for_role(
    role: SenderVisualRole,
    wire_image_data: Option<super::wire_hints::WireImageData>,
    materialized_content: Option<ImageData>,
) -> (Option<ImageData>, Option<ImageData>) {
    match role {
        SenderVisualRole::ConversationAvatar => {
            // Communication artwork becomes a small sender visual before model storage
            let sender_visual = wire_image_data
                .and_then(|image| image.into_storage_image(MAX_STORED_AVATAR_DIMENSION));
            (materialized_content, sender_visual)
        }
        // Non-communication artwork uses the larger content-image storage bound
        SenderVisualRole::ApplicationProvidedIcon | SenderVisualRole::None => {
            let content_image = wire_image_data
                .and_then(|image| image.into_storage_image(MAX_STORED_CONTENT_DIMENSION))
                .or(materialized_content);
            (content_image, None)
        }
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
    if !may_materialize_content_image(attribution) {
        return None;
    }
    let path = image_path
        .filter(|path| !path.trim().is_empty())
        .map(str::to_owned)?;
    run_avatar_worker(
        move || materialize_sender_visual(&path, MAX_STORED_CONTENT_DIMENSION),
        CONVERSATION_AVATAR_TIMEOUT,
    )
    .await
    .flatten()
}

#[cfg(test)]
#[path = "tests/flow.rs"]
mod tests;
