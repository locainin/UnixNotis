//! Stat grid and card construction

use gtk::prelude::*;
use gtk::Align;
use unixnotis_core::{css::hooks, StatWidgetConfig};

use super::{stats_builtin::BuiltinStat, StatGrid, StatItem};

impl StatGrid {
    pub fn new(configs: &[StatWidgetConfig]) -> Option<Self> {
        let mut items = Vec::new();
        for config in configs {
            if !config.enabled {
                continue;
            }
            // Preserve config order so layout remains predictable for users
            items.push(StatItem::new(config.clone()));
        }
        if items.is_empty() {
            // Skip widget creation when all stat entries are disabled
            return None;
        }

        let root = gtk::FlowBox::new();
        root.add_css_class(hooks::stat_card::GRID);
        root.set_selection_mode(gtk::SelectionMode::None);
        root.set_max_children_per_line(2);
        root.set_min_children_per_line(2);
        root.set_row_spacing(8);
        root.set_column_spacing(8);
        root.set_halign(Align::Fill);
        root.set_hexpand(true);

        for item in &items {
            // Insert in order so per-widget identity stays stable
            root.insert(&item.root, -1);
        }

        Some(Self { root, items })
    }

    pub fn root(&self) -> &gtk::FlowBox {
        &self.root
    }

    pub fn refresh(&self, base_interval: std::time::Duration, force: bool) {
        for item in &self.items {
            // Per-item refresh keeps slow widgets from blocking the grid
            item.refresh(base_interval, force);
        }
    }

    pub fn next_refresh_in(&self, now: std::time::Instant) -> Option<std::time::Duration> {
        self.items
            .iter()
            .filter_map(|item| item.next_refresh_in(now))
            .min()
    }

    pub fn is_due(&self, now: std::time::Instant) -> bool {
        self.next_refresh_in(now)
            .map(|delay| delay.is_zero())
            .unwrap_or(false)
    }
}

impl StatItem {
    pub(super) fn new(config: StatWidgetConfig) -> Self {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
        card.add_css_class(hooks::stat_card::ROOT);
        if config.plugin.is_some() {
            // Plugin cards get a dedicated class so themes can separate them from builtin stats
            card.add_css_class(hooks::stat_card::PLUGIN);
        } else {
            card.add_css_class(hooks::stat_card::BUILTIN);
        }
        if config.min_height > 0 {
            // Respect configured min height to keep cards visually aligned
            card.set_size_request(-1, config.min_height);
        }

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        header.add_css_class(hooks::stat_card::HEADER);
        if let Some(icon_name) = config.icon.as_ref() {
            let icon = gtk::Image::from_icon_name(icon_name);
            icon.set_pixel_size(16);
            icon.add_css_class(hooks::stat_card::ICON);
            header.append(&icon);
            card.add_css_class(hooks::stat_card::HAS_ICON);
        } else {
            // No-icon cards still expose a hook so spacing can be rebalanced in CSS
            card.add_css_class(hooks::stat_card::NO_ICON);
        }

        let title = gtk::Label::new(Some(&config.label));
        title.add_css_class(hooks::stat_card::TITLE);
        title.set_xalign(0.0);
        header.append(&title);

        let value_label = gtk::Label::new(Some("n/a"));
        value_label.add_css_class(hooks::stat_card::VALUE);
        value_label.set_xalign(0.0);
        value_label.set_width_chars(12);

        card.append(&header);
        card.append(&value_label);

        let builtin = if config.plugin.is_some() {
            // Plugin-backed stats bypass builtin readers to avoid dual data sources
            None
        } else {
            config
                .cmd
                .as_ref()
                .and_then(|cmd| BuiltinStat::from_command(cmd))
        };

        Self {
            config,
            // Card widgets and refresh state stay together so one item owns its full lifecycle
            root: card,
            value_label,
            builtin: std::rc::Rc::new(std::cell::RefCell::new(builtin)),
            inflight: std::rc::Rc::new(std::cell::Cell::new(false)),
            last_value: std::rc::Rc::new(std::cell::RefCell::new(None)),
            refresh_backoff: std::rc::Rc::new(std::cell::RefCell::new(
                super::RefreshBackoff::default(),
            )),
        }
    }
}
