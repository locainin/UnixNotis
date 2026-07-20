//! Statistic card construction

use gtk::prelude::*;
use unixnotis_core::{css::hooks, IconAssetResolver, StatWidgetConfig};

use super::super::builtin::BuiltinStat;
use super::super::style::stat_kind_css_class;
use super::StatItem;
use crate::ui::widgets::icon_image::image_from_icon_config;
use crate::ui::widgets::utils::RefreshBackoff;

impl StatItem {
    pub(in crate::ui::widgets::stats) fn new(
        config: StatWidgetConfig,
        icon_resolver: &IconAssetResolver,
    ) -> Self {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
        card.add_css_class(hooks::stat_card::ROOT);
        if config.plugin.is_some() {
            // Plugin cards expose a dedicated theme hook
            card.add_css_class(hooks::stat_card::PLUGIN);
        } else {
            card.add_css_class(hooks::stat_card::BUILTIN);
        }
        if config.min_height > 0 {
            // A minimum height keeps cards aligned within the grid
            card.set_size_request(-1, config.min_height);
        }
        if let Some(kind) = config.kind.as_deref().and_then(stat_kind_css_class) {
            // Kind hooks allow stable theme targeting without relying on order
            card.add_css_class(&kind);
        }

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        header.add_css_class(hooks::stat_card::HEADER);
        if let Some(icon) = image_from_icon_config(
            icon_resolver,
            &config.label,
            config.icon.as_deref(),
            config.icon_asset.as_deref(),
            16,
        ) {
            icon.add_css_class(hooks::stat_card::ICON);
            header.append(&icon);
            card.add_css_class(hooks::stat_card::HAS_ICON);
        } else {
            // No-icon cards expose a hook so CSS can rebalance spacing
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
            // Plugin-backed cards bypass built-in readers
            None
        } else {
            config
                .cmd
                .as_ref()
                .and_then(|cmd| cmd.program())
                .and_then(|program| program.to_str())
                .and_then(BuiltinStat::from_command)
        };

        Self {
            config,
            root: card,
            value_label,
            builtin: std::rc::Rc::new(std::cell::RefCell::new(builtin)),
            inflight: std::rc::Rc::new(std::cell::Cell::new(false)),
            last_value: std::rc::Rc::new(std::cell::RefCell::new(None)),
            refresh_backoff: std::rc::Rc::new(std::cell::RefCell::new(RefreshBackoff::default())),
        }
    }

    pub(in crate::ui::widgets::stats) const fn root(&self) -> &gtk::Box {
        &self.root
    }
}
