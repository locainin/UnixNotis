//! Notification row widget module
//!
//! `mod.rs` only wires the notification row pieces together
//! Build, reply, state, and update logic stay in focused modules

mod build;
mod reply;
mod stack;
mod state;
#[cfg(test)]
#[path = "tests/support.rs"]
mod test_support;
mod update;

// The list factory only needs the stable notification-row entry points
// Re-export them here so callers do not need to know the internal file split
pub(in crate::ui::notifications) use self::build::build_notification_row;
pub(in crate::ui::notifications) use self::state::NotificationRowWidgets;
pub(in crate::ui::notifications) use self::update::{
    clear_notification_row, update_notification_row,
};
