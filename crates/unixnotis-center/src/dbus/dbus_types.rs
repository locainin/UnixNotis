//! Shared UI event and command types for the center D-Bus runtime.

use unixnotis_core::{CloseReason, ControlState, Margins, NotificationView, PanelRequest};

use crate::media::MediaInfo;

/// Events delivered to the GTK main loop.
#[derive(Debug, Clone)]
pub enum UiEvent {
    Seed {
        state: ControlState,
        active: Vec<NotificationView>,
        history: Vec<NotificationView>,
    },
    NotificationAdded(NotificationView, bool),
    NotificationUpdated(NotificationView, bool),
    NotificationClosed(u32, CloseReason),
    StateChanged(ControlState),
    PanelRequested(PanelRequest),
    GroupToggled(String),
    /// Updated set of active media players for the widget.
    MediaUpdated(Vec<MediaInfo>),
    MediaCleared,
    /// Hyprland active-window change that may indicate a click-away.
    ClickOutside,
    /// Hyprland reserved work area update for panel sizing.
    WorkAreaUpdated(Option<Margins>),
    RefreshWidgets,
    CssReload,
    ConfigReload,
}

/// Commands sent from GTK handlers to the D-Bus runtime.
#[derive(Debug, Clone)]
pub enum UiCommand {
    Dismiss(u32),
    InvokeAction { id: u32, action_key: String },
    ClearAll,
    SetDnd(bool),
    ClosePanel,
}
