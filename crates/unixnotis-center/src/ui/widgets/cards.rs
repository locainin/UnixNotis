//! Card-style widgets for summary content.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{glib, Align};
use tracing::warn;
use unixnotis_core::{CardWidgetConfig, PanelDebugLevel};

use super::util::{run_command_capture_async, RefreshBackoff};
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
    // Backoff reduces repeated command executions when the value is stable.
    refresh_backoff: Rc<RefCell<RefreshBackoff>>,
    // Calendar only changes daily; track the last rendered day to avoid redundant updates.
    last_calendar_day: Rc<Cell<Option<(i32, i32, i32)>>>,
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
        root.set_row_spacing(8);
        root.set_column_spacing(8);
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

    pub fn refresh(&self, base_interval: Duration, force: bool) {
        for item in &self.items {
            item.refresh(base_interval, force);
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
            refresh_backoff: Rc::new(RefCell::new(RefreshBackoff::default())),
            last_calendar_day: Rc::new(Cell::new(None)),
        }
    }

    fn refresh(&self, base_interval: Duration, force: bool) {
        if self.is_calendar {
            debug::log(PanelDebugLevel::Verbose, || "calendar refresh".to_string());
            let now = Instant::now();
            // Skip calendar refresh while within the backoff window.
            if !self
                .refresh_backoff
                .borrow()
                .should_refresh(now, force)
            {
                return;
            }
            self.refresh_calendar(base_interval);
            return;
        }
        if !self.root.is_visible() {
            return;
        }
        let now = Instant::now();
        // Skip command execution while within the backoff window.
        if !self
            .refresh_backoff
            .borrow()
            .should_refresh(now, force)
        {
            return;
        }
        debug::log(PanelDebugLevel::Verbose, || {
            format!("card refresh: {}", self.config.title)
        });
        let Some(cmd) = self.config.cmd.as_ref() else {
            // Static cards do not need repeated refresh work once visible.
            self.refresh_backoff
                .borrow_mut()
                .note_success(Instant::now(), base_interval, false);
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
        let refresh_backoff = self.refresh_backoff.clone();
        glib::MainContext::default().spawn_local(async move {
            let output = match rx.recv().await {
                Ok(output) => output,
                Err(_) => {
                    inflight.set(false);
                    refresh_backoff
                        .borrow_mut()
                        .note_error(Instant::now(), base_interval);
                    return;
                }
            };
            inflight.set(false);
            let output = match output {
                Ok(output) => output,
                Err(err) => {
                    warn!(?cmd, ?err, "info card command failed");
                    apply_cached_value(&label, &last_value);
                    refresh_backoff
                        .borrow_mut()
                        .note_error(Instant::now(), base_interval);
                    return;
                }
            };
            if !output.status.success() {
                warn!(?cmd, "info card command failed");
                apply_cached_value(&label, &last_value);
                refresh_backoff
                    .borrow_mut()
                    .note_error(Instant::now(), base_interval);
                return;
            }
            let stdout = String::from_utf8_lossy(&output.stdout);
            let value = stdout.trim();
            if value.is_empty() {
                apply_cached_value(&label, &last_value);
                refresh_backoff
                    .borrow_mut()
                    .note_success(Instant::now(), base_interval, false);
            } else {
                let changed = last_value.borrow().as_deref() != Some(value);
                if changed {
                    label.set_text(value);
                    *last_value.borrow_mut() = Some(value.to_string());
                }
                refresh_backoff
                    .borrow_mut()
                    .note_success(Instant::now(), base_interval, changed);
            }
        });
    }

    fn refresh_calendar(&self, base_interval: Duration) {
        let Some(calendar) = self.calendar.as_ref() else {
            return;
        };
        match glib::DateTime::now_local() {
            Ok(now) => {
                let date_key = (now.year(), now.month(), now.day_of_month());
                let changed = self.last_calendar_day.get() != Some(date_key);
                if changed {
                    calendar.select_day(&now);
                    self.last_calendar_day.set(Some(date_key));
                }
                self.refresh_backoff
                    .borrow_mut()
                    .note_success(Instant::now(), base_interval, changed);
            }
            Err(err) => {
                warn!(?err, "calendar refresh failed");
                self.refresh_backoff
                    .borrow_mut()
                    .note_error(Instant::now(), base_interval);
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
