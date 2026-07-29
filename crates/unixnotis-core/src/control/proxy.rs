//! Generated D-Bus control proxy contract

// The proxy macro creates signal collections consumed through generated streams
#![expect(
    clippy::collection_is_never_read,
    reason = "the zbus proxy macro generates signal collections consumed through generated streams"
)]

use zbus::proxy;

use crate::{NotificationDiagnosticsView, NotificationView, PopupCandidate};

use super::{
    CloseReason, ControlState, InhibitorInfo, PanelDebugLevel, PanelRequest, PopupGateState,
    UiHealth,
};

#[proxy(
    interface = "com.unixnotis.Control",
    default_service = "com.unixnotis.Control",
    default_path = "/com/unixnotis/Control"
)]
trait Control {
    /// Coordinated private interface version
    fn get_api_version(&self) -> zbus::Result<u32>;
    /// Current daemon state
    fn get_state(&self) -> zbus::Result<ControlState>;
    /// Readiness of the daemon-managed center and popup clients
    fn get_ui_health(&self) -> zbus::Result<UiHealth>;
    /// Active notifications intended for popups
    fn list_active(&self) -> zbus::Result<Vec<NotificationView>>;
    /// Active notifications whose persistent rule policy permits popup rendering
    fn list_popup_candidates(&self) -> zbus::Result<Vec<NotificationView>>;
    /// History notifications for the panel
    fn list_history(&self) -> zbus::Result<Vec<NotificationView>>;
    /// Fetch one currently active notification by identifier
    fn get_active_notification(&self, id: u32) -> zbus::Result<Vec<NotificationView>>;
    /// Fetch one current popup payload and admission decision atomically
    fn get_popup_candidate(&self, id: u32) -> zbus::Result<Vec<PopupCandidate>>;
    /// Explain attribution and popup admission for one active notification
    fn get_notification_diagnostics(
        &self,
        id: u32,
    ) -> zbus::Result<Vec<NotificationDiagnosticsView>>;
    /// Open the control center panel
    fn open_panel(&self) -> zbus::Result<()>;
    /// Open the control center panel with debug logging
    fn open_panel_debug(&self, level: PanelDebugLevel) -> zbus::Result<()>;
    /// Close the control center panel
    fn close_panel(&self) -> zbus::Result<()>;
    /// Toggle the control center panel
    fn toggle_panel(&self) -> zbus::Result<()>;
    /// Update the Do Not Disturb state
    fn set_dnd(&self, enabled: bool) -> zbus::Result<()>;
    /// Enable Do Not Disturb until one future Unix timestamp
    fn set_dnd_until(&self, expires_at: i64) -> zbus::Result<()>;
    /// Toggle the Do Not Disturb state atomically in the daemon
    fn toggle_dnd(&self) -> zbus::Result<()>;
    /// Register an inhibitor and return its token
    fn inhibit(&self, reason: &str, scope: u32) -> zbus::Result<u64>;
    /// Remove a previously registered inhibitor token
    fn uninhibit(&self, id: u64) -> zbus::Result<()>;
    /// List active inhibitors
    fn list_inhibitors(&self) -> zbus::Result<Vec<InhibitorInfo>>;
    /// Remove only the exact notification generation represented by a UI row
    fn dismiss_generation(&self, id: u32, generation: u64) -> zbus::Result<()>;
    /// Invoke an action only for the exact notification generation represented by a UI row
    fn invoke_action_generation(
        &self,
        id: u32,
        generation: u64,
        action_key: &str,
    ) -> zbus::Result<()>;
    /// Submit text for an explicitly advertised inline-reply action
    fn reply_notification(&self, id: u32, generation: u64, reply_text: &str) -> zbus::Result<()>;
    /// Clear active notifications and saved history
    fn clear_all(&self) -> zbus::Result<()>;
    /// Clear active notifications without deleting saved history
    fn clear_active(&self) -> zbus::Result<()>;
    /// Clear saved history without closing active notifications
    fn clear_history(&self) -> zbus::Result<()>;
    /// Mark the panel UI ready after signal subscriptions are active
    fn mark_panel_ready(&self) -> zbus::Result<()>;
    /// Clear panel readiness while the UI reconnects or shuts down
    #[zbus(no_autostart)]
    fn mark_panel_not_ready(&self) -> zbus::Result<()>;
    /// Mark popup rendering ready after subscriptions, seed, and GTK initialization
    fn mark_popups_ready(&self) -> zbus::Result<()>;
    /// Clear popup readiness during orderly shutdown without activating the daemon
    #[zbus(no_autostart)]
    fn mark_popups_not_ready(&self) -> zbus::Result<()>;
    /// Confirm that GTK attached one exact generation to the popup stack
    fn mark_popup_materialized(&self, id: u32, generation: u64) -> zbus::Result<()>;
    /// Confirm that one exact generation became visible on a mapped popup surface
    fn mark_popup_visible(&self, id: u32, generation: u64) -> zbus::Result<()>;

    #[zbus(signal)]
    fn notification_added(&self, id: u32, generation: u64) -> zbus::Result<()>;
    #[zbus(signal)]
    fn notification_updated(&self, id: u32, generation: u64) -> zbus::Result<()>;
    #[zbus(signal)]
    fn notification_closed(
        &self,
        id: u32,
        generation: u64,
        reason: CloseReason,
    ) -> zbus::Result<()>;
    #[zbus(signal)]
    fn state_changed(&self, state: ControlState) -> zbus::Result<()>;
    /// Emitted only when popup gating changes
    #[zbus(signal)]
    fn popup_gate_changed(&self, gate: PopupGateState) -> zbus::Result<()>;
    /// Emitted when local notification snapshots must refresh
    #[zbus(signal)]
    fn snapshot_invalidated(&self) -> zbus::Result<()>;
    /// Emitted when inhibitor state or count changes
    #[zbus(signal)]
    fn inhibitors_changed(&self, active: bool, count: u32) -> zbus::Result<()>;
    #[zbus(signal)]
    fn panel_requested(&self, request: PanelRequest) -> zbus::Result<()>;
}
