//! Statistic grid construction

use gtk::prelude::*;
use gtk::Align;
use unixnotis_core::{css::hooks, IconAssetResolver, StatWidgetConfig};

use super::super::card::StatItem;
use super::StatGrid;

impl StatGrid {
    pub fn new(
        configs: &[StatWidgetConfig],
        columns: usize,
        icon_resolver: &IconAssetResolver,
    ) -> Option<Self> {
        let mut items = Vec::new();
        for config in configs {
            if !config.enabled {
                continue;
            }
            // Preserve config order so layout remains predictable
            items.push(StatItem::new(config.clone(), icon_resolver));
        }
        if items.is_empty() {
            // Skip widget creation when all stat entries are disabled
            return None;
        }

        let root = gtk::FlowBox::new();
        root.add_css_class(hooks::stat_card::GRID);
        root.set_selection_mode(gtk::SelectionMode::None);
        let columns = flowbox_columns(columns);
        root.set_max_children_per_line(columns);
        root.set_min_children_per_line(columns);
        root.set_row_spacing(8);
        root.set_column_spacing(8);
        root.set_halign(Align::Fill);
        root.set_hexpand(true);

        for item in &items {
            // Insert in order so card identity stays stable
            root.insert(item.root(), -1);
        }

        Some(Self { root, items })
    }

    pub const fn root(&self) -> &gtk::FlowBox {
        &self.root
    }
}

pub(in crate::ui::widgets::stats) fn flowbox_columns(columns: usize) -> u32 {
    u32::try_from(columns.max(1)).unwrap_or(u32::MAX)
}
