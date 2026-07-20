//! Mirrored tests for notification row updates

mod actions;
mod labels;
mod metadata;
mod stack;
mod state;
mod thumbnail;

pub(super) use super::actions::{clamp_action_label_text, visible_action_count};
pub(super) use super::labels::optional_label_state;
pub(super) use super::metadata::{
    notification_meta_label, relative_time_badge, relative_time_badge_at,
};
pub(super) use super::row::update_notification_row;
pub(super) use super::thumbnail::notification_has_thumbnail;
pub(super) use super::visual::{stack_ghost_visibility, StackGhostVisibility};
