//! Notification collection mutation, lifecycle, and row diffing

mod blocks;
mod lifecycle;
mod mutation;
mod update;

pub(in crate::ui::notifications) use super::model::{item, types};
pub(in crate::ui::notifications) use item::RowItem;
