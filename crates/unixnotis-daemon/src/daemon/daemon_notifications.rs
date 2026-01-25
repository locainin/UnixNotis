//! D-Bus server for org.freedesktop.Notifications.
//!
//! Keeps notification delivery logic separate from the control-plane interface.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tracing::debug;
use unixnotis_core::{
    Action, CloseReason, Config, Notification, NotificationImage, Urgency,
    CONTROL_OBJECT_PATH,
};
use zbus::zvariant::OwnedValue;
use zbus::{interface, SignalContext};

use crate::expire::ExpirationScheduler;

use super::{DaemonState, ControlServer, NOTIFICATIONS_OBJECT_PATH, to_fdo_error};

/// D-Bus server for org.freedesktop.Notifications.
pub struct NotificationServer {
    state: Arc<DaemonState>,
    scheduler: ExpirationScheduler,
}

impl NotificationServer {
    pub fn new(state: Arc<DaemonState>, scheduler: ExpirationScheduler) -> Self {
        Self { state, scheduler }
    }
}

#[interface(name = "org.freedesktop.Notifications")]
impl NotificationServer {
    async fn get_capabilities(&self) -> Vec<String> {
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
        expire_timeout: i32,
    ) -> zbus::fdo::Result<u32> {
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
        let notification = build_notification(
            app_name,
            app_icon,
            summary,
            body,
            actions,
            hints,
            expire_timeout,
        );

        let (outcome, expiration) = {
            let mut store = self.state.store.lock().await;
            let outcome = store.insert(notification, replaces_id);
            let expiration = if outcome.dropped {
                None
            } else {
                let expiration = resolve_expiration(store.config(), &outcome.notification);
                store.set_expiration(outcome.notification.id, expiration);
                expiration
            };
            (outcome, expiration)
        };
        if outcome.dropped {
            // Drop-all mode skips scheduling, sound, and control-plane signals entirely.
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
        // Sound playback is driven by hints plus configured defaults.
        self.state
            .sound
            .play_from_hints(&outcome.notification.hints, outcome.allow_sound);

        let control_ctx = SignalContext::new(self.state.connection(), CONTROL_OBJECT_PATH)
            .map_err(to_fdo_error)?;
        if outcome.replaced {
            ControlServer::notification_updated(
                &control_ctx,
                outcome.notification.to_view(),
                outcome.show_popup,
            )
            .await
            .map_err(to_fdo_error)?;
        } else {
            ControlServer::notification_added(
                &control_ctx,
                outcome.notification.to_view(),
                outcome.show_popup,
            )
            .await
            .map_err(to_fdo_error)?;
        }
        // Evictions occur when active history limits are exceeded.
        self.handle_evicted(outcome.evicted).await?;
        self.state
            .emit_state_changed()
            .await
            .map_err(to_fdo_error)?;

        Ok(outcome.notification.id)
    }

    async fn handle_evicted(&self, evicted: Vec<u32>) -> zbus::fdo::Result<()> {
        if evicted.is_empty() {
            return Ok(());
        }
        // Emit close signals for evicted notifications to keep UI state consistent.
        let notif_ctx = SignalContext::new(self.state.connection(), NOTIFICATIONS_OBJECT_PATH)
            .map_err(to_fdo_error)?;
        let control_ctx = SignalContext::new(self.state.connection(), CONTROL_OBJECT_PATH)
            .map_err(to_fdo_error)?;
        for id in evicted {
            NotificationServer::notification_closed(&notif_ctx, id, CloseReason::Undefined as u32)
                .await
                .map_err(to_fdo_error)?;
            ControlServer::notification_closed(&control_ctx, id, CloseReason::Undefined)
                .await
                .map_err(to_fdo_error)?;
        }
        Ok(())
    }

    async fn close_notification(&self, id: u32) -> zbus::fdo::Result<()> {
        debug!(id, "close notification requested");
        self.state
            .close_notification(id, CloseReason::ClosedByCall)
            .await
            .map_err(to_fdo_error)
    }

    async fn get_server_information(&self) -> (String, String, String, String) {
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

fn build_notification(
    app_name: String,
    app_icon: String,
    summary: String,
    body: String,
    actions: Vec<String>,
    hints: HashMap<String, OwnedValue>,
    expire_timeout: i32,
) -> Notification {
    // Derive common hints first so the UI and rule engine can make decisions.
    let urgency = Urgency::from_hint(hints.get("urgency"));
    let category = hints.get("category").and_then(owned_to_string);
    let is_transient = hints
        .get("transient")
        .and_then(|value| bool::try_from(value).ok())
        .unwrap_or(false);
    let is_resident = hints
        .get("resident")
        .and_then(|value| bool::try_from(value).ok())
        .unwrap_or(false);
    let image = NotificationImage::from_hints(&app_name, &app_icon, &hints);

    Notification {
        id: 0,
        app_name: if app_name.is_empty() {
            "Unknown".to_string()
        } else {
            app_name
        },
        app_icon,
        summary,
        body,
        actions: parse_actions(actions),
        hints,
        urgency,
        category,
        is_transient,
        is_resident,
        suppress_popup: false,
        suppress_sound: false,
        image,
        expire_timeout,
        received_at: chrono::Utc::now(),
    }
}

fn parse_actions(raw: Vec<String>) -> Vec<Action> {
    let mut actions = Vec::new();
    let mut iter = raw.into_iter();
    // D-Bus actions arrive as [key, label] pairs; drop any trailing key without a label.
    while let Some(key) = iter.next() {
        if let Some(label) = iter.next() {
            actions.push(Action { key, label });
        }
    }
    actions
}

fn resolve_expiration(config: &Config, notification: &Notification) -> Option<Instant> {
    // Explicit timeouts and resident notifications override defaults.
    if notification.expire_timeout == 0 || notification.is_resident {
        return None;
    }

    let timeout_ms = if notification.expire_timeout > 0 {
        notification.expire_timeout as u64
    } else {
        match notification.urgency {
            Urgency::Critical => config.popups.critical_timeout_ms?,
            _ => config.popups.default_timeout_ms,
        }
    };

    if timeout_ms == 0 {
        return None;
    }

    // Convert the resolved timeout into an absolute instant for the scheduler.
    Some(Instant::now() + Duration::from_millis(timeout_ms))
}

fn owned_to_string(value: &OwnedValue) -> Option<String> {
    value
        .try_clone()
        .ok()
        .and_then(|owned| String::try_from(owned).ok())
}
