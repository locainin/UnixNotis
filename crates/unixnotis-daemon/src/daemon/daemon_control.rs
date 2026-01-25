//! D-Bus server for com.unixnotis.Control.
//!
//! Provides panel control, state queries, and inhibitor management.

use std::sync::Arc;

use futures_util::stream::{FuturesUnordered, StreamExt};
use tracing::warn;
use unixnotis_core::{
    CloseReason, ControlState, InhibitorInfo, NotificationView, PanelDebugLevel, PanelRequest,
    CONTROL_OBJECT_PATH,
};
use zbus::fdo::DBusProxy;
use zbus::message::Header;
use zbus::{interface, SignalContext};

use super::{to_fdo_error, DaemonState, NotificationServer, NOTIFICATIONS_OBJECT_PATH};

/// D-Bus server for com.unixnotis.Control.
pub struct ControlServer {
    state: Arc<DaemonState>,
}

impl ControlServer {
    pub fn new(state: Arc<DaemonState>) -> Self {
        Self { state }
    }
}

#[interface(name = "com.unixnotis.Control")]
impl ControlServer {
    async fn get_state(&self) -> ControlState {
        let store = self.state.store.lock().await;
        // Read cached inhibitor values to keep state queries constant-time.
        ControlState {
            dnd_enabled: store.dnd_enabled(),
            history_count: store.history_len() as u32,
            inhibited: store.inhibited(),
            inhibitor_count: store.inhibitor_count(),
        }
    }

    async fn list_active(&self) -> Vec<NotificationView> {
        let store = self.state.store.lock().await;
        // Active list is used for popup seeding and panel hydration.
        store.list_active()
    }

    async fn list_history(&self) -> Vec<NotificationView> {
        let store = self.state.store.lock().await;
        // History list is used for panel hydration and pagination.
        store.list_history()
    }

    async fn open_panel(&self) -> zbus::fdo::Result<()> {
        let ctx = SignalContext::new(self.state.connection(), CONTROL_OBJECT_PATH)
            .map_err(to_fdo_error)?;
        // Use a signal to keep UI and daemon loosely coupled.
        ControlServer::panel_requested(&ctx, PanelRequest::open())
            .await
            .map_err(to_fdo_error)
    }

    async fn open_panel_debug(&self, level: PanelDebugLevel) -> zbus::fdo::Result<()> {
        let ctx = SignalContext::new(self.state.connection(), CONTROL_OBJECT_PATH)
            .map_err(to_fdo_error)?;
        // Debug open keeps the same panel request path with extra verbosity metadata.
        ControlServer::panel_requested(&ctx, PanelRequest::open_debug(level))
            .await
            .map_err(to_fdo_error)
    }

    async fn close_panel(&self) -> zbus::fdo::Result<()> {
        let ctx = SignalContext::new(self.state.connection(), CONTROL_OBJECT_PATH)
            .map_err(to_fdo_error)?;
        // Close is a signal so the UI can apply its own visibility rules.
        ControlServer::panel_requested(&ctx, PanelRequest::close())
            .await
            .map_err(to_fdo_error)
    }

    async fn toggle_panel(&self) -> zbus::fdo::Result<()> {
        let ctx = SignalContext::new(self.state.connection(), CONTROL_OBJECT_PATH)
            .map_err(to_fdo_error)?;
        // Toggle is emitted as a request to avoid tight coupling to UI state.
        ControlServer::panel_requested(&ctx, PanelRequest::toggle())
            .await
            .map_err(to_fdo_error)
    }

    async fn set_dnd(&self, enabled: bool) -> zbus::fdo::Result<()> {
        let changed = {
            let mut store = self.state.store.lock().await;
            store.set_dnd(enabled)
        };
        if changed {
            // Emit state updates only when the value changes to avoid log noise.
            self.state
                .emit_state_changed()
                .await
                .map_err(to_fdo_error)?;
        }
        Ok(())
    }

    async fn inhibit(
        &self,
        reason: &str,
        scope: u32,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<u64> {
        let sender = header
            .sender()
            .ok_or_else(|| zbus::fdo::Error::Failed("missing sender".to_string()))?;
        // Track inhibitors by unique bus name so cleanup on disconnect is reliable.
        let (id, active, count) = {
            let mut store = self.state.store.lock().await;
            let id = store.add_inhibitor(sender.to_string(), reason.to_string(), scope);
            let active = store.inhibited();
            let count = store.inhibitor_count();
            (id, active, count)
        };
        let ctx = SignalContext::new(self.state.connection(), CONTROL_OBJECT_PATH)
            .map_err(to_fdo_error)?;
        // Emit inhibitors_changed before state_changed so UIs can update counts immediately.
        ControlServer::inhibitors_changed(&ctx, active, count)
            .await
            .map_err(to_fdo_error)?;
        self.state
            .emit_state_changed()
            .await
            .map_err(to_fdo_error)?;
        Ok(id)
    }

    async fn uninhibit(
        &self,
        id: u64,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<()> {
        let sender = header
            .sender()
            .ok_or_else(|| zbus::fdo::Error::Failed("missing sender".to_string()))?;
        let owner = sender.to_string();
        // Owner checks prevent one client from clearing another client's inhibitor.
        let (removed, active, count) = {
            let mut store = self.state.store.lock().await;
            match store.remove_inhibitor(id, &owner) {
                Ok(removed) => {
                    let active = store.inhibited();
                    let count = store.inhibitor_count();
                    (removed, active, count)
                }
                Err(err) => {
                    return Err(zbus::fdo::Error::AccessDenied(err.message()));
                }
            }
        };
        if !removed {
            // Unknown IDs are treated as a no-op to keep clients resilient.
            return Ok(());
        }
        let ctx = SignalContext::new(self.state.connection(), CONTROL_OBJECT_PATH)
            .map_err(to_fdo_error)?;
        // Broadcast inhibitor updates so UI clients can refresh badges.
        ControlServer::inhibitors_changed(&ctx, active, count)
            .await
            .map_err(to_fdo_error)?;
        self.state
            .emit_state_changed()
            .await
            .map_err(to_fdo_error)?;
        Ok(())
    }

    async fn list_inhibitors(&self) -> Vec<InhibitorInfo> {
        let store = self.state.store.lock().await;
        // Returned list is already sorted for deterministic output.
        store.list_inhibitors()
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
        // Action signals re-use the freedesktop notification interface path.
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
        // Emit close signals concurrently to avoid blocking on large clears.
        let mut tasks = FuturesUnordered::new();
        for id in ids {
            let notif_ctx = notif_ctx.clone();
            let control_ctx = control_ctx.clone();
            tasks.push(async move {
                NotificationServer::notification_closed(
                    &notif_ctx,
                    id,
                    CloseReason::DismissedByUser as u32,
                )
                .await?;
                ControlServer::notification_closed(&control_ctx, id, CloseReason::DismissedByUser)
                    .await?;
                Ok::<(), zbus::Error>(())
            });
        }
        while let Some(result) = tasks.next().await {
            result.map_err(to_fdo_error)?;
        }
        self.state.emit_state_changed().await.map_err(to_fdo_error)
    }

    #[zbus(signal)]
    pub(crate) async fn notification_added(
        ctx: &SignalContext<'_>,
        notification: NotificationView,
        show_popup: bool,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub(crate) async fn notification_updated(
        ctx: &SignalContext<'_>,
        notification: NotificationView,
        show_popup: bool,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub(crate) async fn notification_closed(
        ctx: &SignalContext<'_>,
        id: u32,
        reason: CloseReason,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub(crate) async fn state_changed(
        ctx: &SignalContext<'_>,
        state: ControlState,
    ) -> zbus::Result<()>;

    /// Emitted when inhibitor state toggles or count changes.
    #[zbus(signal)]
    pub(crate) async fn inhibitors_changed(
        ctx: &SignalContext<'_>,
        active: bool,
        count: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub(crate) async fn panel_requested(
        ctx: &SignalContext<'_>,
        request: PanelRequest,
    ) -> zbus::Result<()>;
}

pub async fn spawn_inhibitor_owner_watch(state: Arc<DaemonState>) -> zbus::Result<()> {
    let proxy = DBusProxy::new(state.connection()).await?;
    let mut stream = proxy.receive_name_owner_changed().await?;
    tokio::spawn(async move {
        while let Some(signal) = stream.next().await {
            let args = match signal.args() {
                Ok(args) => args,
                Err(err) => {
                    warn!(?err, "failed to decode NameOwnerChanged args");
                    continue;
                }
            };
            if args.new_owner().is_some() {
                continue;
            }
            let owner = args.name().to_string();
            // Drop inhibitors for disconnected clients to avoid stale suppression.
            let (changed, active, count) = {
                let mut store = state.store.lock().await;
                let changed = store.remove_inhibitors_by_owner(&owner);
                let active = store.inhibited();
                let count = store.inhibitor_count();
                (changed, active, count)
            };
            if !changed {
                continue;
            }
            let ctx = match SignalContext::new(state.connection(), CONTROL_OBJECT_PATH) {
                Ok(ctx) => ctx,
                Err(err) => {
                    warn!(?err, "failed to build signal context for inhibitor cleanup");
                    continue;
                }
            };
            if let Err(err) = ControlServer::inhibitors_changed(&ctx, active, count).await {
                warn!(
                    ?err,
                    "failed to emit inhibitors_changed after owner disconnect"
                );
            }
            if let Err(err) = state.emit_state_changed().await {
                warn!(?err, "failed to emit state_changed after owner disconnect");
            }
        }
    });
    Ok(())
}
