//! Control D-Bus interface implementation

use std::sync::Arc;

use unixnotis_core::{
    CloseReason, ControlState, InhibitorInfo, NotificationView, PanelDebugLevel, PanelRequest,
    PopupGateState,
};
use zbus::message::Header;
use zbus::{interface, SignalContext};

use crate::daemon::{auth, to_fdo_error, DaemonState};

/// D-Bus server for com.unixnotis.Control
pub struct ControlServer {
    // Shared daemon state used by all control methods
    // The server stays thin
    pub(super) state: Arc<DaemonState>,
}

impl ControlServer {
    pub const fn new(state: Arc<DaemonState>) -> Self {
        // Lightweight wrapper around the shared daemon state
        Self { state }
    }

    pub(super) async fn authorize_control_call(
        &self,
        header: &Header<'_>,
        method: &'static str,
    ) -> zbus::fdo::Result<()> {
        // One auth path
        auth::authorize_control_call(&self.state, header, method).await
    }

    pub(super) async fn authorize_panel_readiness_call(
        &self,
        header: &Header<'_>,
        method: &'static str,
    ) -> zbus::fdo::Result<()> {
        // Panel readiness is restricted to unixnotis-center identity
        auth::authorize_panel_readiness_call(&self.state, header, method).await
    }

    pub(super) fn ensure_panel_available(&self) -> zbus::fdo::Result<()> {
        // Rejecting here makes panel outages visible instead of silent
        if self.state.panel_ready() {
            return Ok(());
        }
        Err(zbus::fdo::Error::Failed(
            "unixnotis-center is unavailable".to_string(),
        ))
    }

    pub(super) async fn drain_active_notifications(&self) -> Vec<u32> {
        let ids = {
            let mut store = self.state.store.lock().await;
            store.drain_active_ids()
        };
        self.state.cancel_expirations(&ids);
        ids
    }

    pub(super) async fn clear_saved_history(&self) {
        let mut store = self.state.store.lock().await;
        store.clear_history();
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
        self.state.apply_dnd_state(enabled).await
    }

    pub(super) async fn set_dnd_until(
        &self,
        expires_at: i64,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<()> {
        self.authorize_control_call(&header, "SetDndUntil").await?;
        self.state.apply_dnd_until(expires_at).await
    }

    async fn toggle_dnd(&self, #[zbus(header)] header: Header<'_>) -> zbus::fdo::Result<()> {
        self.authorize_control_call(&header, "ToggleDnd").await?;
        self.state.apply_toggle_dnd().await
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

    pub(super) async fn invoke_action(
        &self,
        id: u32,
        action_key: &str,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<()> {
        self.authorize_control_call(&header, "InvokeAction").await?;
        self.invoke_validated_action(id, action_key).await
    }

    pub(super) async fn reply_notification(
        &self,
        id: u32,
        reply_text: &str,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<()> {
        self.authorize_control_call(&header, "ReplyNotification")
            .await?;
        self.submit_inline_reply(id, reply_text).await
    }

    pub(super) async fn clear_all(
        &self,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<()> {
        self.authorize_control_call(&header, "ClearAll").await?;
        let ids = self.drain_active_notifications().await;
        self.clear_saved_history().await;
        self.state.publish_notifications_cleared(ids).await;
        Ok(())
    }

    pub(super) async fn clear_active(
        &self,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<()> {
        self.authorize_control_call(&header, "ClearActive").await?;
        let ids = self.drain_active_notifications().await;
        self.state.publish_notifications_cleared(ids).await;
        Ok(())
    }

    pub(super) async fn clear_history(
        &self,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<()> {
        self.authorize_control_call(&header, "ClearHistory").await?;
        self.clear_saved_history().await;
        self.state.publish_notifications_cleared(Vec::new()).await;
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

    /// Emitted when inhibitor state toggles or count changes
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
