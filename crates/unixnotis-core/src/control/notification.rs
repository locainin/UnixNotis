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
#[derive(Debug, Copy, Clone, Default, Eq, PartialEq, Serialize_repr, Deserialize_repr, Type)]
#[repr(u8)]
pub enum PopupAdmissionView {
    Show = 0,
    Rule = 1,
    Dnd = 2,
    Inhibitor = 3,
    #[default]
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

/// Furthest delivery stage reached by one committed popup decision
#[derive(Debug, Copy, Clone, Default, Eq, PartialEq, Serialize_repr, Deserialize_repr, Type)]
#[repr(u8)]
pub enum PopupDeliveryStage {
    #[default]
    Suppressed = 0,
    Admitted = 1,
    FanoutFailed = 2,
    RendererFetched = 3,
    Materialized = 4,
    Visible = 5,
}

impl PopupDeliveryStage {
    /// Monotonic ordering for retained delivery history
    #[must_use]
    pub const fn rank(self) -> u8 {
        self as u8
    }
}

/// Immutable arrival decision plus later delivery progress for one generation
#[derive(Debug, Clone, Default, Eq, PartialEq, Serialize, Deserialize, Type)]
pub struct PopupDecisionRecord {
    pub admission_at_commit: PopupAdmissionView,
    pub renderer_process_running_at_commit: bool,
    pub renderer_ready_at_commit: bool,
    /// Readiness revision observed while the notification was committed
    pub renderer_health_revision_at_commit: u64,
    pub max_visible_at_commit: u32,
    pub decided_at_unix_ms: i64,
    pub delivery_stage: PopupDeliveryStage,
    /// Sanitized banner visibility duration fixed for this notification generation
    pub popup_hide_after_ms: u64,
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
