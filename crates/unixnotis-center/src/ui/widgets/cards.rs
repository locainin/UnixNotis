//! Card-style widgets for summary content.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{glib, Align};
use tracing::warn;
use unixnotis_core::{CardWidgetConfig, PanelDebugLevel};

use super::util::run_command_capture_async;
use crate::debug;

pub struct CardGrid {
    root: gtk::FlowBox,
    items: Vec<CardItem>,
}

struct CardItem {
    config: CardWidgetConfig,
    root: gtk::Box,
    body_label: gtk::Label,
    calendar: Option<gtk::Calendar>,
    is_calendar: bool,
    inflight: Rc<Cell<bool>>,
    last_value: Rc<RefCell<Option<String>>>,
}

impl CardGrid {
    pub fn new(configs: &[CardWidgetConfig]) -> Option<Self> {
        let mut items = Vec::new();
        for config in configs {
            if !config.enabled {
                continue;
            }
            items.push(CardItem::new(config.clone()));
        }
        if items.is_empty() {
            return None;
        }

        let root = gtk::FlowBox::new();
        root.add_css_class("unixnotis-card-grid");
        root.set_selection_mode(gtk::SelectionMode::None);
        root.set_max_children_per_line(2);
        root.set_min_children_per_line(2);
        root.set_row_spacing(10);
        root.set_column_spacing(10);
        root.set_halign(Align::Fill);
        root.set_hexpand(true);

        for item in &items {
            root.insert(&item.root, -1);
        }

        Some(Self { root, items })
    }

    pub fn root(&self) -> &gtk::FlowBox {
        &self.root
    }

    pub fn refresh(&self) {
        for item in &self.items {
            item.refresh();
        }
    }
}

impl CardItem {
    fn new(config: CardWidgetConfig) -> Self {
        let is_calendar = matches!(config.kind.as_deref(), Some("calendar"));
        let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
        root.add_css_class("unixnotis-info-card");
        if config.monospace {
            root.add_css_class("unixnotis-info-card-mono");
        }
        if let Some(kind) = config.kind.as_deref() {
            match kind {
                "calendar" => root.add_css_class("unixnotis-info-card-calendar"),
                "weather" => root.add_css_class("unixnotis-info-card-weather"),
                _ => {}
            }
        }
        if config.min_height > 0 {
            root.set_size_request(-1, config.min_height);
        }

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        header.add_css_class("unixnotis-info-header");
        if let Some(icon_name) = config.icon.as_ref() {
            let icon = gtk::Image::from_icon_name(icon_name);
            if matches!(config.kind.as_deref(), Some("weather")) {
                icon.set_pixel_size(24);
                icon.add_css_class("unixnotis-info-icon-weather");
            } else {
                icon.set_pixel_size(18);
            }
            icon.add_css_class("unixnotis-info-icon");
            header.append(&icon);
        }

        let title = gtk::Label::new(Some(&config.title));
        title.add_css_class("unixnotis-info-title");
        title.set_xalign(0.0);
        header.append(&title);

        let body_label = gtk::Label::new(Some(config.subtitle.as_deref().unwrap_or("")));
        body_label.add_css_class("unixnotis-info-body");
        body_label.set_xalign(0.0);
        body_label.set_wrap(true);
        body_label.set_wrap_mode(gtk::pango::WrapMode::WordChar);

        root.append(&header);
        let calendar = if is_calendar {
            let calendar = gtk::Calendar::new();
            calendar.add_css_class("unixnotis-calendar");
            calendar.set_hexpand(true);
            calendar.set_vexpand(false);
            calendar.set_halign(Align::Fill);
            calendar.set_valign(Align::Start);
            root.append(&calendar);
            Some(calendar)
        } else {
            root.append(&body_label);
            None
        };

        Self {
            config,
            root,
            body_label,
            calendar,
            is_calendar,
            inflight: Rc::new(Cell::new(false)),
            last_value: Rc::new(RefCell::new(None)),
        }
    }

    fn refresh(&self) {
        if self.is_calendar {
            debug::log(PanelDebugLevel::Verbose, || "calendar refresh".to_string());
            self.refresh_calendar();
            return;
        }
        if !self.root.is_visible() {
            return;
        }
        debug::log(PanelDebugLevel::Verbose, || {
            format!("card refresh: {}", self.config.title)
        });
        let Some(cmd) = self.config.cmd.as_ref() else {
            return;
        };
        if self.inflight.get() {
            return;
        }
        self.inflight.set(true);
        let cmd = cmd.clone();
        let rx = run_command_capture_async(&cmd);
        let label = self.body_label.clone();
        let inflight = self.inflight.clone();
        let last_value = self.last_value.clone();
        glib::MainContext::default().spawn_local(async move {
            let output = match rx.recv().await {
                Ok(output) => output,
                Err(_) => {
                    inflight.set(false);
                    return;
                }
            };
            inflight.set(false);
            let output = match output {
                Ok(output) => output,
                Err(err) => {
                    warn!(?cmd, ?err, "info card command failed");
                    apply_cached_value(&label, &last_value);
                    return;
                }
            };
            if !output.status.success() {
                warn!(?cmd, "info card command failed");
                apply_cached_value(&label, &last_value);
                return;
            }
            let stdout = String::from_utf8_lossy(&output.stdout);
            let value = stdout.trim();
            if value.is_empty() {
                apply_cached_value(&label, &last_value);
            } else {
                if last_value.borrow().as_deref() == Some(value) {
                    return;
                }
                label.set_text(value);
                *last_value.borrow_mut() = Some(value.to_string());
            }
        });
    }

    fn refresh_calendar(&self) {
        let Some(calendar) = self.calendar.as_ref() else {
            return;
        };
        match glib::DateTime::now_local() {
            Ok(now) => calendar.select_day(&now),
            Err(err) => {
                warn!(?err, "calendar refresh failed");
            }
        }
    }
}

fn apply_cached_value(label: &gtk::Label, cache: &Rc<RefCell<Option<String>>>) {
    if let Some(value) = cache.borrow().as_ref() {
        if label.text().as_str() != value {
            label.set_text(value);
        }
    } else if label.text().as_str() != "n/a" {
        label.set_text("n/a");
    }
}
