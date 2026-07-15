//! Notification visibility and archive policy

use crate::Urgency;

use super::{CloseReason, ControlState};

/// Reports whether a notification may appear as a popup
#[must_use]
pub const fn popup_allowed_by_state(urgency: u8, state: &ControlState) -> bool {
    // Inhibitors hide all popups no matter what the notification says
    if state.inhibited {
        return false;
    }
    // DND still allows critical popups so urgent issues stay visible
    if state.dnd_enabled {
        return urgency == Urgency::Critical as u8;
    }
    true
}

/// Reports whether a closed notification belongs in history
#[must_use]
pub const fn should_archive_closed_notification(
    close_reason: CloseReason,
    is_transient: bool,
    transient_to_history: bool,
) -> bool {
    // User dismissal removes the row instead of archiving it
    if matches!(close_reason, CloseReason::DismissedByUser) {
        return false;
    }
    // Transient rows enter history only when configuration allows it
    if is_transient && !transient_to_history {
        return false;
    }
    true
}

#[cfg(test)]
#[path = "tests/policy.rs"]
mod tests;
