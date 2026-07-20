//! Notification list state, grouping, and GTK row rendering
//!
//! The folder root stays focused on module wiring and the notification-list surface

mod model;
mod row;
mod store;
#[cfg(test)]
#[path = "tests/support.rs"]
pub(super) mod test_support;
mod view;

pub(in crate::ui) use model::types::NotificationCounts;
pub use model::types::{NotificationList, NotificationListConfig};

pub(in crate::ui::notifications) use model::item;
