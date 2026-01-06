//! D-Bus server implementation and daemon state coordination.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;
use tracing::{debug, info};
use unixnotis_core::{
    Action, CloseReason, Config, Notification, NotificationImage, NotificationView, PanelDebugLevel,
    PanelRequest, Urgency, CONTROL_BUS_NAME, CONTROL_OBJECT_PATH,
};
use zbus::fdo::{RequestNameFlags, RequestNameReply};
use zbus::zvariant::OwnedValue;
use zbus::{interface, Connection, SignalContext};

use crate::expire::ExpirationScheduler;
use crate::sound::SoundSettings;
use crate::store::NotificationStore;

const NOTIFICATIONS_OBJECT_PATH: &str = "/org/freedesktop/Notifications";

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
            }
        };
        let control_ctx = SignalContext::new(&self.connection, CONTROL_OBJECT_PATH)?;
        ControlServer::state_changed(&control_ctx, state).await
    }

    fn connection(&self) -> &Connection {
        &self.connection
    }
}

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

/// D-Bus server for com.unixnotis.Control.
pub struct ControlServer {
    state: Arc<DaemonState>,
}

impl ControlServer {
    pub fn new(state: Arc<DaemonState>) -> Self {
        Self { state }
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
        debug!(
            app_name,
            summary, replaces_id, expire_timeout, "received notification"
        );
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
            let expiration = resolve_expiration(store.config(), &outcome.notification);
            store.set_expiration(outcome.notification.id, expiration);
            (outcome, expiration)
        };
        self.scheduler.schedule(outcome.notification.id, expiration);
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
    async fn notification_closed(ctx: &SignalContext<'_>, id: u32, reason: u32)
        -> zbus::Result<()>;

    #[zbus(signal)]
    async fn action_invoked(ctx: &SignalContext<'_>, id: u32, action_key: &str)
        -> zbus::Result<()>;
}

#[interface(name = "com.unixnotis.Control")]
impl ControlServer {
    async fn get_state(&self) -> unixnotis_core::ControlState {
        let store = self.state.store.lock().await;
        unixnotis_core::ControlState {
            dnd_enabled: store.dnd_enabled(),
            history_count: store.history_len() as u32,
        }
    }

    async fn list_active(&self) -> Vec<NotificationView> {
        let store = self.state.store.lock().await;
        store.list_active()
    }

    async fn list_history(&self) -> Vec<NotificationView> {
        let store = self.state.store.lock().await;
        store.list_history()
    }

    async fn open_panel(&self) -> zbus::fdo::Result<()> {
        let ctx = SignalContext::new(self.state.connection(), CONTROL_OBJECT_PATH)
            .map_err(to_fdo_error)?;
        ControlServer::panel_requested(&ctx, PanelRequest::open())
            .await
            .map_err(to_fdo_error)
    }

    async fn open_panel_debug(&self, level: PanelDebugLevel) -> zbus::fdo::Result<()> {
        let ctx = SignalContext::new(self.state.connection(), CONTROL_OBJECT_PATH)
            .map_err(to_fdo_error)?;
        ControlServer::panel_requested(&ctx, PanelRequest::open_debug(level))
            .await
            .map_err(to_fdo_error)
    }

    async fn close_panel(&self) -> zbus::fdo::Result<()> {
        let ctx = SignalContext::new(self.state.connection(), CONTROL_OBJECT_PATH)
            .map_err(to_fdo_error)?;
        ControlServer::panel_requested(&ctx, PanelRequest::close())
            .await
            .map_err(to_fdo_error)
    }

    async fn toggle_panel(&self) -> zbus::fdo::Result<()> {
        let ctx = SignalContext::new(self.state.connection(), CONTROL_OBJECT_PATH)
            .map_err(to_fdo_error)?;
        ControlServer::panel_requested(&ctx, PanelRequest::toggle())
            .await
            .map_err(to_fdo_error)
    }

    async fn set_dnd(&self, enabled: bool) -> zbus::fdo::Result<()> {
        {
            let mut store = self.state.store.lock().await;
            store.set_dnd(enabled);
        }
        self.state.emit_state_changed().await.map_err(to_fdo_error)
    }

    async fn dismiss(&self, id: u32) -> zbus::fdo::Result<()> {
        self.state
            .dismiss_from_panel(id)
            .await
            .map_err(to_fdo_error)
    }

    async fn invoke_action(&self, id: u32, action_key: &str) -> zbus::fdo::Result<()> {
        let ctx = SignalContext::new(self.state.connection(), NOTIFICATIONS_OBJECT_PATH)
            .map_err(to_fdo_error)?;
        NotificationServer::action_invoked(&ctx, id, action_key)
            .await
            .map_err(to_fdo_error)
    }

    async fn clear_all(&self) -> zbus::fdo::Result<()> {
        // Drain active notifications in one lock to avoid quadratic scans.
        let ids = {
            let mut store = self.state.store.lock().await;
            let ids = store.drain_active_ids();
            store.clear_history();
            ids
        };
        if ids.is_empty() {
            return self.state.emit_state_changed().await.map_err(to_fdo_error);
        }
        let notif_ctx = SignalContext::new(self.state.connection(), NOTIFICATIONS_OBJECT_PATH)
            .map_err(to_fdo_error)?;
        let control_ctx = SignalContext::new(self.state.connection(), CONTROL_OBJECT_PATH)
            .map_err(to_fdo_error)?;
        for id in ids {
            NotificationServer::notification_closed(
                &notif_ctx,
                id,
                CloseReason::DismissedByUser as u32,
            )
            .await
            .map_err(to_fdo_error)?;
            ControlServer::notification_closed(&control_ctx, id, CloseReason::DismissedByUser)
                .await
                .map_err(to_fdo_error)?;
        }
        self.state.emit_state_changed().await.map_err(to_fdo_error)
    }

    #[zbus(signal)]
    async fn notification_added(
        ctx: &SignalContext<'_>,
        notification: NotificationView,
        show_popup: bool,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn notification_updated(
        ctx: &SignalContext<'_>,
        notification: NotificationView,
        show_popup: bool,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn notification_closed(
        ctx: &SignalContext<'_>,
        id: u32,
        reason: CloseReason,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn state_changed(
        ctx: &SignalContext<'_>,
        state: unixnotis_core::ControlState,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn panel_requested(ctx: &SignalContext<'_>, request: PanelRequest) -> zbus::Result<()>;
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

    Some(Instant::now() + Duration::from_millis(timeout_ms))
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

fn owned_to_string(value: &OwnedValue) -> Option<String> {
    value
        .try_clone()
        .ok()
        .and_then(|owned| String::try_from(owned).ok())
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

fn to_fdo_error(err: zbus::Error) -> zbus::fdo::Error {
    zbus::fdo::Error::Failed(err.to_string())
}
