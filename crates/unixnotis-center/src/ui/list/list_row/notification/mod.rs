//! Notification row widget module
//!
//! `mod.rs` only wires the notification row pieces together
//! Build, state, update, and tests stay in their own files

mod build;
mod state;
#[cfg(test)]
mod tests;
mod update;

// The list factory only needs the stable notification-row entry points
// Re-export them here so callers do not need to know the internal file split
pub(in crate::ui::list) use self::build::build_notification_row;
pub(in crate::ui::list) use self::state::NotificationRowWidgets;
pub(in crate::ui::list) use self::update::update_notification_row;
