//! Statistic grid ownership

pub(in crate::ui::widgets::stats) mod build;
mod refresh;
pub(in crate::ui::widgets::stats) mod schedule;

use super::card::StatItem;

pub struct StatGrid {
    // FlowBox root is embedded by the panel widget tree
    root: gtk::FlowBox,
    // Per-card state is retained for refresh scheduling
    items: Vec<StatItem>,
}
