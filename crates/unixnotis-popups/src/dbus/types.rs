//! D-Bus-facing popup event and command types

use unixnotis_core::{CloseReason, ControlState, NotificationView, PopupGateState};

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
    NotificationClosed(u32, CloseReason),
    // Popup gate is split out so panel-only state changes do not wake the popup UI
    PopupGateChanged(PopupGateState),
    CssReload,
    ConfigReload,
}

/// Commands sent from GTK handlers to the D-Bus runtime
#[derive(Debug, Clone)]
pub enum UiCommand {
    Dismiss(u32),
    InvokeAction { id: u32, action_key: String },
    // A synchronous acknowledgement lets GTK wait for MarkPopupsNotReady before process exit
    Shutdown(std::sync::mpsc::SyncSender<()>),
}

#[cfg(test)]
#[path = "tests/types.rs"]
mod tests;
