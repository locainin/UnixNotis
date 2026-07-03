//! Notification list state, grouping, and GTK row rendering
//!
//! The folder root stays focused on module wiring and the public list surface

mod blocks;
mod build;
mod grouping;
mod index;
mod item;
mod lifecycle;
mod mutation;
mod row;
#[cfg(test)]
#[path = "tests/support.rs"]
pub(super) mod test_support;
mod types;
mod update;
mod widgets;

pub use self::types::{NotificationList, NotificationListConfig};

pub(super) use self::item::RowItem;
