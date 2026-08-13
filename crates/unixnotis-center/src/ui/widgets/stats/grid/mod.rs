//! Statistic grid ownership

pub(in crate::ui::widgets::stats) mod build;
mod model;
mod refresh;
pub(in crate::ui::widgets::stats) mod schedule;

pub use model::StatGrid;
