//! D-Bus-facing popup event and command types

use unixnotis_core::{
    CloseReason, ControlState, NotificationKey, NotificationView, PopupGateState,
};

/// Events delivered to the GTK main loop
#[derive(Debug, Clone)]
pub enum UiEvent {
    // Owner loss clears every popup from the previous daemon generation
    Disconnected,
    Seed {
        state: ControlState,
        active: Vec<NotificationView>,
    },
    // Add and update reuse the shared lightweight NotificationView payload
    NotificationAdded(NotificationView, bool),
    NotificationUpdated(NotificationView, bool),
    NotificationClosed(NotificationKey, CloseReason),
    // Popup gate is split out so panel-only state changes do not wake the popup UI
    PopupGateChanged(PopupGateState),
    CssReload,
    ConfigReload,
}

/// Commands sent from GTK handlers to the D-Bus runtime
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
    Materialized(NotificationKey),
    Visible(NotificationKey),
}

impl std::fmt::Debug for UiCommand {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
                // Reply text is private message content and must never enter debug logs
                .field("text", &"<redacted>")
                .finish_non_exhaustive(),
            Self::Materialized(notification) => formatter
                .debug_tuple("Materialized")
                .field(notification)
                .finish(),
            Self::Visible(notification) => formatter
                .debug_tuple("Visible")
                .field(notification)
                .finish(),
        }
    }
}

#[cfg(test)]
#[path = "tests/types.rs"]
mod tests;
