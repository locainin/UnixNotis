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

/// One atomic popup payload and its current admission decision
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Type)]
pub struct PopupCandidate {
    pub notification: NotificationView,
    pub should_show: bool,
}
