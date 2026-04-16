//! D-Bus server for com.unixnotis.Control.
//!
//! Provides panel control, state queries, and inhibitor management.

use std::sync::Arc;

use unixnotis_core::{
    CloseReason, ControlState, InhibitorInfo, NotificationView, PanelDebugLevel, PanelRequest,
    PopupGateState,
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
// Split DND mutation/persistence flow out so control interface methods stay small
// DND helpers live there
#[path = "daemon_control/dnd.rs"]
mod dnd;
// Split query/read methods out so interface declarations stay compact
// Query helpers live there
#[path = "daemon_control/query.rs"]
mod query;
// Split panel request/readiness flow out so panel lifecycle behavior is isolated
// Panel helpers live there
#[path = "daemon_control/panel.rs"]
mod panel;
// Split inhibitor mutation/fanout flow out so concurrency behavior is isolated
// Inhibitor helpers live there
#[path = "daemon_control/inhibit.rs"]
mod inhibit;

/// D-Bus server for com.unixnotis.Control.
pub struct ControlServer {
    // Shared daemon state used by all control methods
    // The server stays thin
    state: Arc<DaemonState>,
}
// Cap inhibitor count so memory use stays bounded even under abusive clients
const MAX_ACTIVE_INHIBITORS: u32 = 128;
#[cfg(test)]
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

    async fn authorize_panel_readiness_call(
        &self,
        header: &Header<'_>,
        method: &'static str,
    ) -> zbus::fdo::Result<()> {
        // Panel readiness is restricted to unixnotis-center identity
        auth::authorize_panel_readiness_call(&self.state, header, method).await
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
}

#[interface(name = "com.unixnotis.Control")]
impl ControlServer {
    async fn get_state(
        &self,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<ControlState> {
        self.query_state(&header).await
    }

    async fn list_active(
        &self,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<Vec<NotificationView>> {
        self.query_active(&header).await
    }

    async fn list_history(
        &self,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<Vec<NotificationView>> {
        self.query_history(&header).await
    }

    async fn get_active_notification(
        &self,
        id: u32,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<Vec<NotificationView>> {
        self.query_active_notification(id, &header).await
    }

    async fn open_panel(&self, #[zbus(header)] header: Header<'_>) -> zbus::fdo::Result<()> {
        self.request_panel_command(&header, "OpenPanel", PanelRequest::open())
            .await
    }

    async fn open_panel_debug(
        &self,
        level: PanelDebugLevel,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<()> {
        self.request_panel_command(&header, "OpenPanelDebug", PanelRequest::open_debug(level))
            .await
    }

    async fn close_panel(&self, #[zbus(header)] header: Header<'_>) -> zbus::fdo::Result<()> {
        self.request_panel_command(&header, "ClosePanel", PanelRequest::close())
            .await
    }

    async fn toggle_panel(&self, #[zbus(header)] header: Header<'_>) -> zbus::fdo::Result<()> {
        self.request_panel_command(&header, "TogglePanel", PanelRequest::toggle())
            .await
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
        self.apply_toggle_dnd().await
    }

    async fn inhibit(
        &self,
        reason: &str,
        scope: u32,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<u64> {
        self.apply_inhibit(reason, scope, &header).await
    }

    async fn uninhibit(
        &self,
        id: u64,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<()> {
        self.apply_uninhibit(id, &header).await
    }

    async fn list_inhibitors(
        &self,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<Vec<InhibitorInfo>> {
        self.query_inhibitors(&header).await
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
        self.set_panel_ready_state(&header, "MarkPanelReady", true)
            .await
    }

    async fn mark_panel_not_ready(
        &self,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<()> {
        self.set_panel_ready_state(&header, "MarkPanelNotReady", false)
            .await
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
