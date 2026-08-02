//! Notification row update wiring

mod actions;
mod labels;
mod metadata;
mod row;
mod thumbnail;
mod visual;

pub(in crate::ui::notifications) use row::{clear_notification_row, update_notification_row};

#[cfg(test)]
mod tests;
