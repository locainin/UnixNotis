//! Notification close reason wire types

use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use zbus::zvariant::Type;

use crate::NotificationView;

/// Reason codes aligned with the notification specification
#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize_repr, Deserialize_repr, Type)]
#[repr(u32)]
pub enum CloseReason {
    Expired = 1,
    DismissedByUser = 2,
    ClosedByCall = 3,
    Undefined = 4,
}

/// Current reason a stored notification may or may not become a popup
#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize_repr, Deserialize_repr, Type)]
#[repr(u8)]
pub enum PopupAdmissionView {
    Show = 0,
    Rule = 1,
    Dnd = 2,
    Inhibitor = 3,
    RendererUnavailable = 4,
    RendererDisabled = 5,
}

impl PopupAdmissionView {
    /// Whether the current admission permits popup rendering
    #[must_use]
    pub const fn should_show(self) -> bool {
        matches!(self, Self::Show)
    }
}

/// One atomic popup payload and its current admission decision
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Type)]
pub struct PopupCandidate {
    pub notification: NotificationView,
    pub admission: PopupAdmissionView,
}

#[cfg(test)]
#[path = "tests/notification.rs"]
mod tests;
