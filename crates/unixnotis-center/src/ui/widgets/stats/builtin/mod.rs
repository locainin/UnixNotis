//! Built-in statistic sources and refresh infrastructure

mod detect;
pub(in crate::ui::widgets::stats) mod group;
mod model;
pub(in crate::ui::widgets::stats) mod readers;
pub(in crate::ui::widgets::stats) mod worker;

pub(super) use model::{BuiltinStat, BuiltinStatKey};
