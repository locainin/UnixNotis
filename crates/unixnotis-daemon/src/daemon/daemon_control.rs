//! D-Bus server for com.unixnotis.Control.
//!
//! Provides panel control, state queries, and inhibitor management.

use std::sync::Arc;

use tracing::warn;
use unixnotis_core::{
    CloseReason, ControlState, InhibitorInfo, NotificationView, PanelDebugLevel, PanelRequest,
    PopupGateState, CONTROL_OBJECT_PATH,
};
use zbus::message::Header;
use zbus::{interface, SignalContext};

use super::{to_fdo_error, DaemonState, NotificationServer, NOTIFICATIONS_OBJECT_PATH};

// Split auth logic out so this file stays focused on control interface behavior
// Auth checks live there
#[path = "daemon_control/auth.rs"]
mod auth;
// Split clear-all fanout out so signal planning does not crowd the interface methods
// Clear-all logic lives there
#[path = "daemon_control/clear.rs"]
mod clear;
// Split input normalization out so validation is shared and easy to test
// Input cleanup lives there
#[path = "daemon_control/sanitize.rs"]
mod sanitize;
// Split owner-watch logic out so background cleanup code is isolated
// Owner watch lives there
#[path = "daemon_control/watch.rs"]
mod watch;

/// D-Bus server for com.unixnotis.Control.
pub struct ControlServer {
    // Shared daemon state used by all control methods
    // The server stays thin
    state: Arc<DaemonState>,
}
// Cap inhibitor count so memory use stays bounded even under abusive clients
const MAX_ACTIVE_INHIBITORS: u32 = 128;
use clear::clear_all_signal_plan;

impl ControlServer {
    pub fn new(state: Arc<DaemonState>) -> Self {
        // Lightweight wrapper around the shared daemon state
        Self { state }
    }

    async fn authorize_control_call(
        &self,
        header: &Header<'_>,
        method: &'static str,
    ) -> zbus::fdo::Result<()> {
        // One auth path
        auth::authorize_control_call(&self.state, header, method).await
    }

    fn ensure_panel_available(&self) -> zbus::fdo::Result<()> {
        // Rejecting here makes panel outages visible instead of silent
        if self.state.panel_ready() {
            return Ok(());
        }
        Err(zbus::fdo::Error::Failed(
            "unixnotis-center is unavailable".to_string(),
        ))
    }

    async fn apply_dnd_state(&self, enabled: bool) -> zbus::fdo::Result<()> {
        // Capture the previous state so persistence failures can roll back cleanly
        let (changed, persist, previous) = {
            let mut store = self.state.store.lock().await;
            let previous = store.dnd_enabled();
            let (changed, persist) = store.set_dnd(enabled);
            (changed, persist, previous)
        };
        if let Some(store) = persist {
            // Persist outside the main store lock to avoid blocking notify paths on I/O
            if let Err(err) = store.persist(enabled) {
                warn!(?err, "failed to persist do-not-disturb state");
                // Undo the in-memory change
                let mut state = self.state.store.lock().await;
                let _ = state.set_dnd(previous);
                return Err(zbus::fdo::Error::Failed(
                    "failed to persist do-not-disturb state".to_string(),
                ));
            }
        }
        if changed {
            // Emit state updates only when the value changes to avoid log noise.
            self.state
                .emit_state_changed()
                .await
                .map_err(to_fdo_error)?;
        }
        Ok(())
    }
}

#[interface(name = "com.unixnotis.Control")]
impl ControlServer {
    async fn get_state(
        &self,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<ControlState> {
        // State metadata is now treated as privileged control telemetry
        self.authorize_control_call(&header, "GetState").await?;
        // Single lock read keeps state snapshot internally consistent
        let store = self.state.store.lock().await;
        // Cheap state snapshot
        Ok(ControlState {
            dnd_enabled: store.dnd_enabled(),
            history_count: store.history_len() as u32,
            inhibited: store.inhibited(),
            inhibitor_count: store.inhibitor_count(),
        })
    }

    async fn list_active(
        &self,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<Vec<NotificationView>> {
        // Guard against untrusted callers before reading any notification content
        self.authorize_control_call(&header, "ListActive").await?;
        let store = self.state.store.lock().await;
        // Return active items
        Ok(store.list_active())
    }

    async fn list_history(
        &self,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<Vec<NotificationView>> {
        // History can contain sensitive content, so it uses the same auth gate
        self.authorize_control_call(&header, "ListHistory").await?;
        let store = self.state.store.lock().await;
        // Return saved items
        Ok(store.list_history())
    }

    async fn get_active_notification(
        &self,
        id: u32,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<Vec<NotificationView>> {
        // Per-notification fetch keeps full content on an authenticated pull path
        self.authorize_control_call(&header, "GetActiveNotification")
            .await?;
        let store = self.state.store.lock().await;
        Ok(store.active_notification_view(id).into_iter().collect())
    }

    async fn open_panel(&self, #[zbus(header)] header: Header<'_>) -> zbus::fdo::Result<()> {
        self.authorize_control_call(&header, "OpenPanel").await?;
        self.ensure_panel_available()?;
        let ctx = SignalContext::new(self.state.connection(), CONTROL_OBJECT_PATH)
            .map_err(to_fdo_error)?;
        // Use a signal to keep UI and daemon loosely coupled.
        ControlServer::panel_requested(&ctx, PanelRequest::open())
            .await
            .map_err(to_fdo_error)
    }

    async fn open_panel_debug(
        &self,
        level: PanelDebugLevel,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<()> {
        self.authorize_control_call(&header, "OpenPanelDebug")
            .await?;
        self.ensure_panel_available()?;
        let ctx = SignalContext::new(self.state.connection(), CONTROL_OBJECT_PATH)
            .map_err(to_fdo_error)?;
        // Debug open keeps the same panel request path with extra verbosity metadata.
        ControlServer::panel_requested(&ctx, PanelRequest::open_debug(level))
            .await
            .map_err(to_fdo_error)
    }

    async fn close_panel(&self, #[zbus(header)] header: Header<'_>) -> zbus::fdo::Result<()> {
        self.authorize_control_call(&header, "ClosePanel").await?;
        self.ensure_panel_available()?;
        let ctx = SignalContext::new(self.state.connection(), CONTROL_OBJECT_PATH)
            .map_err(to_fdo_error)?;
        // Close is a signal so the UI can apply its own visibility rules.
        ControlServer::panel_requested(&ctx, PanelRequest::close())
            .await
            .map_err(to_fdo_error)
    }

    async fn toggle_panel(&self, #[zbus(header)] header: Header<'_>) -> zbus::fdo::Result<()> {
        self.authorize_control_call(&header, "TogglePanel").await?;
        self.ensure_panel_available()?;
        let ctx = SignalContext::new(self.state.connection(), CONTROL_OBJECT_PATH)
            .map_err(to_fdo_error)?;
        // Toggle is emitted as a request to avoid tight coupling to UI state.
        ControlServer::panel_requested(&ctx, PanelRequest::toggle())
            .await
            .map_err(to_fdo_error)
    }

    async fn set_dnd(
        &self,
        enabled: bool,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<()> {
        self.authorize_control_call(&header, "SetDnd").await?;
        self.apply_dnd_state(enabled).await
    }

    async fn toggle_dnd(&self, #[zbus(header)] header: Header<'_>) -> zbus::fdo::Result<()> {
        self.authorize_control_call(&header, "ToggleDnd").await?;
        let next = {
            let store = self.state.store.lock().await;
            !store.dnd_enabled()
        };
        self.apply_dnd_state(next).await
    }

    async fn inhibit(
        &self,
        reason: &str,
        scope: u32,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<u64> {
        self.authorize_control_call(&header, "Inhibit").await?;
        let sender = header
            .sender()
            .ok_or_else(|| zbus::fdo::Error::Failed("missing sender".to_string()))?;
        // Clean caller input first
        let normalized_scope = sanitize::normalize_inhibit_scope(scope)?;
        let sanitized_reason = sanitize::sanitize_inhibit_reason(reason);
        // Track inhibitors by unique bus name so cleanup on disconnect is reliable.
        let (id, active, count) = {
            let mut store = self.state.store.lock().await;
            if store.inhibitor_count() >= MAX_ACTIVE_INHIBITORS {
                // Hard cap blocks unbounded growth from accidental loops or hostile callers
                return Err(zbus::fdo::Error::Failed(format!(
                    "inhibitor limit reached ({MAX_ACTIVE_INHIBITORS})"
                )));
            }
            let id = store.add_inhibitor(sender.to_string(), sanitized_reason, normalized_scope);
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
        // Uninhibit trusts ownership on the bus sender, not executable allowlists
        let sender = header
            .sender()
            .ok_or_else(|| zbus::fdo::Error::Failed("missing sender".to_string()))?;
        let owner = sender.to_string();
        // Only the owner can remove it
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

    async fn list_inhibitors(
        &self,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<Vec<InhibitorInfo>> {
        self.authorize_control_call(&header, "ListInhibitors")
            .await?;
        let store = self.state.store.lock().await;
        // Returned list is already sorted for deterministic output.
        Ok(store.list_inhibitors())
    }

    async fn dismiss(&self, id: u32, #[zbus(header)] header: Header<'_>) -> zbus::fdo::Result<()> {
        self.authorize_control_call(&header, "Dismiss").await?;
        // Delegate to shared state helper so all close signals stay consistent
        self.state
            .dismiss_from_panel(id)
            .await
            .map_err(to_fdo_error)
    }

    async fn invoke_action(
        &self,
        id: u32,
        action_key: &str,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<()> {
        self.authorize_control_call(&header, "InvokeAction").await?;
        // Reuse the freedesktop action signal path for compatibility with listeners
        let ctx = SignalContext::new(self.state.connection(), NOTIFICATIONS_OBJECT_PATH)
            .map_err(to_fdo_error)?;
        // Action signals re-use the freedesktop notification interface path.
        NotificationServer::action_invoked(&ctx, id, action_key)
            .await
            .map_err(to_fdo_error)
    }

    async fn clear_all(&self, #[zbus(header)] header: Header<'_>) -> zbus::fdo::Result<()> {
        self.authorize_control_call(&header, "ClearAll").await?;
        // Drain active notifications in one lock to avoid quadratic scans.
        let ids = {
            let mut store = self.state.store.lock().await;
            let ids = store.drain_active_ids();
            // Clear live and saved items
            store.clear_history();
            ids
        };
        // Active timers no longer matter once the list has been wiped
        self.state.cancel_expirations(&ids).await;
        // Signal fanout lives in a focused helper so the D-Bus method stays readable
        clear::emit_clear_all_signals(&self.state, ids).await;
        Ok(())
    }

    async fn mark_panel_ready(&self, #[zbus(header)] header: Header<'_>) -> zbus::fdo::Result<()> {
        self.authorize_control_call(&header, "MarkPanelReady")
            .await?;
        // Center calls this only after it is subscribed to panel_requested
        self.state.set_panel_ready(true);
        Ok(())
    }

    async fn mark_panel_not_ready(
        &self,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<()> {
        self.authorize_control_call(&header, "MarkPanelNotReady")
            .await?;
        // Clearing readiness avoids stale success during reconnect windows
        self.state.set_panel_ready(false);
        Ok(())
    }

    #[zbus(signal)]
    pub(crate) async fn notification_added(
        ctx: &SignalContext<'_>,
        id: u32,
        show_popup: bool,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub(crate) async fn notification_updated(
        ctx: &SignalContext<'_>,
        id: u32,
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

    #[zbus(signal)]
    pub(crate) async fn popup_gate_changed(
        ctx: &SignalContext<'_>,
        gate: PopupGateState,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub(crate) async fn snapshot_invalidated(ctx: &SignalContext<'_>) -> zbus::Result<()>;

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
    // Delegate to a focused module so the interface file stays small and readable.
    watch::spawn_inhibitor_owner_watch(state).await
}

#[cfg(test)]
#[path = "daemon_control_tests.rs"]
mod tests;
