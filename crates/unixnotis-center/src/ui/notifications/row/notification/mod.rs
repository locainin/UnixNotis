//! Notification row widget module
//!
//! `mod.rs` only wires the notification row pieces together
//! Reply logic and tests stay inside their focused module

#[cfg(test)]
#[path = "tests/actions.rs"]
mod actions_tests;
mod build;
#[cfg(test)]
#[path = "tests/labels.rs"]
mod labels_tests;
#[cfg(test)]
#[path = "tests/metadata.rs"]
mod metadata_tests;
mod reply;
#[cfg(test)]
#[path = "tests/stack.rs"]
mod stack_tests;
mod state;
#[cfg(test)]
#[path = "tests/state.rs"]
mod state_tests;
#[cfg(test)]
#[path = "tests/support.rs"]
mod test_support;
#[cfg(test)]
#[path = "tests/thumbnail.rs"]
mod thumbnail_tests;
mod update;

// The list factory only needs the stable notification-row entry points
// Re-export them here so callers do not need to know the internal file split
pub(in crate::ui::notifications) use self::build::build_notification_row;
pub(in crate::ui::notifications) use self::state::NotificationRowWidgets;
pub(in crate::ui::notifications) use self::update::update_notification_row;
