//! Inline reply metadata shared by the daemon and notification UIs

use serde::{Deserialize, Serialize};
use zbus::zvariant::Type;

/// KDE-compatible reply controls attached to one notification action
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct InlineReply {
    // False keeps the D-Bus structure stable when no reply action exists
    pub available: bool,
    // Label comes from the matching action pair
    pub label: String,
    // Optional KDE hints use empty strings when the sender omits them
    pub placeholder: String,
    pub submit_label: String,
    pub submit_icon: String,
}

#[cfg(test)]
#[path = "tests/reply.rs"]
mod tests;
