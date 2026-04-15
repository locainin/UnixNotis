//! Card grid and card construction

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;

use gtk::prelude::*;
use gtk::Align;
use unixnotis_core::{css::hooks, CardWidgetConfig};

use super::weather::{apply_card_kind_classes, configure_card_icon};
use super::{CardGrid, CardItem, RefreshBackoff};

impl CardGrid {
    pub fn new(configs: &[CardWidgetConfig]) -> Option<Self> {
        let mut items = Vec::new();
        for config in configs {
            if !config.enabled {
                continue;
            }
            // Preserve config ordering so cards stay in user-defined sequence
            items.push(CardItem::new(config.clone()));
        }
        if items.is_empty() {
            // Skip allocation when all cards are disabled
            return None;
        }

        let root = gtk::FlowBox::new();
        root.add_css_class(hooks::info_card::GRID);
        root.set_selection_mode(gtk::SelectionMode::None);
        root.set_max_children_per_line(2);
        root.set_min_children_per_line(2);
        root.set_row_spacing(8);
        root.set_column_spacing(8);
        root.set_halign(Align::Fill);
        root.set_hexpand(true);

        for item in &items {
            // Insert in config order for deterministic rendering
            root.insert(&item.root, -1);
        }

        Some(Self { root, items })
    }

    pub fn root(&self) -> &gtk::FlowBox {
        &self.root
    }

    pub fn refresh(&self, base_interval: std::time::Duration, force: bool) {
        for item in &self.items {
            // Each card refreshes independently so a slow source does not stall the whole grid
            item.refresh(base_interval, force);
        }
    }

    pub fn next_refresh_in(&self, now: Instant) -> Option<std::time::Duration> {
        self.items
            .iter()
            .filter_map(|item| item.next_refresh_in(now))
            .min()
    }

    pub fn is_due(&self, now: Instant) -> bool {
        self.next_refresh_in(now)
            .map(|delay| delay.is_zero())
            .unwrap_or(false)
    }
}

impl CardItem {
    pub(super) fn new(config: CardWidgetConfig) -> Self {
        let is_calendar = matches!(config.kind.as_deref(), Some("calendar"));
        let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
        root.add_css_class(hooks::info_card::ROOT);
        // Kind classes are applied early so header and body nodes inherit the final card state
        apply_card_kind_classes(&root, &config);
        if config.min_height > 0 {
            root.set_size_request(-1, config.min_height);
        }

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        header.add_css_class(hooks::info_card::HEADER);
        if let Some(icon_name) = config.icon.as_ref() {
            let icon = gtk::Image::from_icon_name(icon_name);
            configure_card_icon(&icon, &config);
            header.append(&icon);
            root.add_css_class(hooks::info_card::HAS_ICON);
        } else {
            // Missing icons get their own hook so weather and text cards can align differently
            root.add_css_class(hooks::info_card::NO_ICON);
        }

        let title_label = gtk::Label::new(Some(&config.title));
        title_label.add_css_class(hooks::info_card::TITLE);
        title_label.set_xalign(0.0);
        header.append(&title_label);

        let body_label = gtk::Label::new(Some(config.subtitle.as_deref().unwrap_or("")));
        body_label.add_css_class(hooks::info_card::BODY);
        body_label.set_xalign(0.0);
        body_label.set_wrap(true);
        body_label.set_wrap_mode(gtk::pango::WrapMode::WordChar);

        root.append(&header);
        let calendar = if is_calendar {
            let calendar = gtk::Calendar::new();
            calendar.add_css_class(hooks::info_card::CALENDAR_WIDGET);
            calendar.set_hexpand(true);
            calendar.set_vexpand(false);
            calendar.set_halign(Align::Fill);
            calendar.set_valign(Align::Start);
            // Calendar cards keep the GTK calendar widget instead of a text body
            root.append(&calendar);
            Some(calendar)
        } else {
            // Non-calendar cards render the normal body label in the same slot
            root.append(&body_label);
            None
        };

        Self {
            config,
            root,
            title_label,
            body_label,
            calendar,
            is_calendar,
            inflight: Rc::new(Cell::new(false)),
            last_value: Rc::new(RefCell::new(None)),
            refresh_backoff: Rc::new(RefCell::new(RefreshBackoff::default())),
            last_calendar_day: Rc::new(Cell::new(None)),
            calendar_next_due: Rc::new(Cell::new(None)),
        }
    }
}
