//! Mirrored tests for inline reply behavior

mod availability;
mod generation;
mod keyboard;
mod motion;
mod presentation;
mod recovery;
mod submission;
mod support;

pub(super) use super::super::build::build_notification_row;
pub(super) use super::super::test_support::{row_data, sample_notification, RowFlags};
pub(super) use super::super::update::update_notification_row;
pub(super) use super::lifecycle::cancel_inline_reply;
pub(super) use super::{build_inline_reply, configure_inline_reply, connect_inline_reply_button};
