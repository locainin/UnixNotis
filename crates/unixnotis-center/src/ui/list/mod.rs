//! Notification list state, grouping, and GTK row rendering
//!
//! The folder root stays focused on module wiring and the public list surface

mod build;
mod list_blocks;
mod list_grouping;
mod list_index;
mod list_item;
mod list_lifecycle;
mod list_mutation;
mod list_row;
mod list_update;
mod list_widgets;
mod types;

pub use self::types::{NotificationList, NotificationListConfig};

pub(super) use self::list_item::RowItem;
