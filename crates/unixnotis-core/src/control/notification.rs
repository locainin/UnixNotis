//! Notification close reason wire types

use serde_repr::{Deserialize_repr, Serialize_repr};
use zbus::zvariant::Type;

/// Reason codes aligned with the notification specification
#[derive(Debug, Copy, Clone, Serialize_repr, Deserialize_repr, Type)]
#[repr(u32)]
pub enum CloseReason {
    Expired = 1,
    DismissedByUser = 2,
    ClosedByCall = 3,
    Undefined = 4,
}
