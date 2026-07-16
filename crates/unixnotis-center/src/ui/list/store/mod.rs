//! Notification collection mutation, lifecycle, and row diffing

mod blocks;
mod lifecycle;
mod mutation;
mod update;

pub(in crate::ui::list) use super::model::{item, types};
pub(in crate::ui::list) use item::RowItem;
