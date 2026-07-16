//! Notification list state, grouping, and GTK row rendering
//!
//! The folder root stays focused on module wiring and the public list surface

mod model;
mod row;
mod store;
#[cfg(test)]
#[path = "tests/support.rs"]
pub(super) mod test_support;
mod view;

pub use model::types::{NotificationList, NotificationListConfig};

pub(in crate::ui::list) use model::item;
#[cfg(test)]
pub(in crate::ui::list) use model::types;
