//! Shared UI event and command types for the center D-Bus runtime.

use std::fmt;

use unixnotis_core::{CloseReason, ControlState, Margins, NotificationView, PanelRequest};

use crate::media::MediaInfo;

/// Events delivered to the GTK main loop.
#[derive(Debug, Clone)]
pub enum UiEvent {
    // Owner loss clears snapshots that belong to the previous daemon generation
    Disconnected,
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
    /// Search/filter query entered in the panel header.
    FilterChanged(String),
    /// Toggle compact mode that hides non-notification widgets.
    WidgetsCollapsed(bool),
    CssReload,
    ConfigReload,
}

/// Commands sent from GTK handlers to the D-Bus runtime.
pub enum UiCommand {
    Dismiss(u32),
    InvokeAction {
        id: u32,
        action_key: String,
    },
    Reply {
        id: u32,
        text: String,
        outcome: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    ClearAll,
    SetDnd(bool),
    SetDndUntil(i64),
    ClosePanel,
}

impl fmt::Debug for UiCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dismiss(id) => formatter.debug_tuple("Dismiss").field(id).finish(),
            Self::InvokeAction { id, action_key } => formatter
                .debug_struct("InvokeAction")
                .field("id", id)
                .field("action_key", action_key)
                .finish(),
            Self::Reply { id, .. } => formatter
                .debug_struct("Reply")
                .field("id", id)
                // Typed message content must never enter diagnostic logs
                .field("text", &"[redacted]")
                .finish_non_exhaustive(),
            Self::ClearAll => formatter.write_str("ClearAll"),
            Self::SetDnd(enabled) => formatter.debug_tuple("SetDnd").field(enabled).finish(),
            Self::SetDndUntil(expires_at) => formatter
                .debug_tuple("SetDndUntil")
                .field(expires_at)
                .finish(),
            Self::ClosePanel => formatter.write_str("ClosePanel"),
        }
    }
}

#[cfg(test)]
#[path = "tests/model.rs"]
mod tests;
