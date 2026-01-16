//! D-Bus control interface types and proxy definitions.

use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use zbus::proxy;
use zbus::zvariant::Type;

use crate::NotificationView;

/// Well-known bus name for the UnixNotis control interface.
pub const CONTROL_BUS_NAME: &str = "com.unixnotis.Control";
/// Object path for control methods and signals.
pub const CONTROL_OBJECT_PATH: &str = "/com/unixnotis/Control";
/// D-Bus interface name for control calls.
pub const CONTROL_INTERFACE: &str = "com.unixnotis.Control";

/// Control-plane state broadcast to the UI.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ControlState {
    pub dnd_enabled: bool,
    pub history_count: u32,
}

/// Panel visibility actions sent to the UI.
#[derive(Debug, Copy, Clone, Serialize_repr, Deserialize_repr, Type)]
#[repr(u32)]
pub enum PanelAction {
    Open = 0,
    Close = 1,
    Toggle = 2,
}

/// Debug verbosity for panel diagnostics requested via control tooling.
#[derive(
    Debug,
    Copy,
    Clone,
    Serialize_repr,
    Deserialize_repr,
    Type,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Default,
)]
#[repr(u8)]
pub enum PanelDebugLevel {
    #[default]
    Off = 0,
    Critical = 1,
    Warn = 2,
    Info = 3,
    Verbose = 4,
}

impl PanelDebugLevel {
    pub fn allows(self, level: PanelDebugLevel) -> bool {
        self != PanelDebugLevel::Off && self >= level
    }
}

/// Panel request payload combining action and requested debug verbosity.
#[derive(Debug, Copy, Clone, Serialize, Deserialize, Type)]
pub struct PanelRequest {
    pub action: PanelAction,
    pub debug: PanelDebugLevel,
}

impl PanelRequest {
    pub fn open() -> Self {
        Self {
            action: PanelAction::Open,
            debug: PanelDebugLevel::Off,
        }
    }

    pub fn open_debug(level: PanelDebugLevel) -> Self {
        Self {
            action: PanelAction::Open,
            debug: level,
        }
    }

    pub fn close() -> Self {
        Self {
            action: PanelAction::Close,
            debug: PanelDebugLevel::Off,
        }
    }

    pub fn toggle() -> Self {
        Self {
            action: PanelAction::Toggle,
            debug: PanelDebugLevel::Off,
        }
    }
}

/// Reason codes aligned with the notification specification.
#[derive(Debug, Copy, Clone, Serialize_repr, Deserialize_repr, Type)]
#[repr(u32)]
pub enum CloseReason {
    Expired = 1,
    DismissedByUser = 2,
    ClosedByCall = 3,
    Undefined = 4,
}

#[proxy(
    interface = "com.unixnotis.Control",
    default_service = "com.unixnotis.Control",
    default_path = "/com/unixnotis/Control"
)]
trait Control {
    /// Current daemon state.
    fn get_state(&self) -> zbus::Result<ControlState>;

    /// Active notifications intended for popups.
    fn list_active(&self) -> zbus::Result<Vec<NotificationView>>;

    /// History notifications for the panel.
    fn list_history(&self) -> zbus::Result<Vec<NotificationView>>;

    /// Open the control center panel.
    fn open_panel(&self) -> zbus::Result<()>;

    /// Open the control center panel with debug logging.
    fn open_panel_debug(&self, level: PanelDebugLevel) -> zbus::Result<()>;

    /// Close the control center panel.
    fn close_panel(&self) -> zbus::Result<()>;

    /// Toggle the control center panel.
    fn toggle_panel(&self) -> zbus::Result<()>;

    /// Update the Do Not Disturb state.
    fn set_dnd(&self, enabled: bool) -> zbus::Result<()>;

    /// Remove a notification by ID.
    fn dismiss(&self, id: u32) -> zbus::Result<()>;

    /// Invoke an action key for a notification.
    fn invoke_action(&self, id: u32, action_key: &str) -> zbus::Result<()>;

    /// Clear all notifications from history and popups.
    fn clear_all(&self) -> zbus::Result<()>;

    #[zbus(signal)]
    fn notification_added(
        &self,
        notification: NotificationView,
        show_popup: bool,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    fn notification_updated(
        &self,
        notification: NotificationView,
        show_popup: bool,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    fn notification_closed(&self, id: u32, reason: CloseReason) -> zbus::Result<()>;

    #[zbus(signal)]
    fn state_changed(&self, state: ControlState) -> zbus::Result<()>;

    #[zbus(signal)]
    fn panel_requested(&self, request: PanelRequest) -> zbus::Result<()>;
}
