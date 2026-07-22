//! Retained statistic grid widget state

use super::super::card::StatItem;

pub struct StatGrid {
    // FlowBox root is embedded by the panel widget tree
    pub(super) root: gtk::FlowBox,
    // Per-card state is retained for refresh scheduling
    pub(super) items: Vec<StatItem>,
}
