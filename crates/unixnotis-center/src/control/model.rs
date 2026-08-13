//! Shared UI event and command types for the center D-Bus runtime.

use std::fmt;

use unixnotis_core::{
    CloseReason, ControlState, Margins, NotificationKey, NotificationView, PanelRequest,
};

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
    NotificationAdded(NotificationView),
    NotificationUpdated(NotificationView),
    NotificationClosed(NotificationKey, CloseReason),
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
    Dismiss(NotificationKey),
    InvokeAction {
        notification: NotificationKey,
        action_key: String,
        confirmed: bool,
    },
    Reply {
        id: u32,
        generation: u64,
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
            Self::Dismiss(notification) => formatter
                .debug_tuple("Dismiss")
                .field(notification)
                .finish(),
            Self::InvokeAction {
                notification,
                action_key,
                confirmed,
            } => formatter
                .debug_struct("InvokeAction")
                .field("notification", notification)
                .field("action_key", action_key)
                .field("confirmed", confirmed)
                .finish(),
            Self::Reply { id, generation, .. } => formatter
                .debug_struct("Reply")
                .field("id", id)
                .field("generation", generation)
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
