//! Mirrored tests for notification row updates

mod actions;
mod labels;
mod metadata;
mod state;
mod thumbnail;
mod visual_matrix;

pub(super) use super::actions::clamp_action_label_text;
pub(super) use super::labels::optional_label_state;
pub(super) use super::metadata::{
    notification_meta_label, relative_time_badge, relative_time_badge_at,
};
pub(super) use super::row::{clear_notification_row, update_notification_row};
