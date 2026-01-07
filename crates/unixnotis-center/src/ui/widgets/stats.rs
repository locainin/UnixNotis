//! Statistic widgets and refresh orchestration.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{glib, Align};
use tracing::warn;
use unixnotis_core::{PanelDebugLevel, StatWidgetConfig};

use super::stats_builtin::BuiltinStat;
use super::util::run_command_capture_async;
use crate::debug;

pub struct StatGrid {
    root: gtk::FlowBox,
    items: Vec<StatItem>,
}

struct StatItem {
    config: StatWidgetConfig,
    root: gtk::Box,
    value_label: gtk::Label,
    builtin: RefCell<Option<BuiltinStat>>,
    inflight: Rc<Cell<bool>>,
    last_value: Rc<RefCell<Option<String>>>,
}

impl StatGrid {
    pub fn new(configs: &[StatWidgetConfig]) -> Option<Self> {
        let mut items = Vec::new();
        for config in configs {
            if !config.enabled {
                continue;
            }
            items.push(StatItem::new(config.clone()));
        }
        if items.is_empty() {
            return None;
        }

        let root = gtk::FlowBox::new();
        root.add_css_class("unixnotis-stat-grid");
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

impl StatItem {
    fn new(config: StatWidgetConfig) -> Self {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
        card.add_css_class("unixnotis-stat-card");
        if config.min_height > 0 {
            card.set_size_request(-1, config.min_height);
        }

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        header.add_css_class("unixnotis-stat-header");
        if let Some(icon_name) = config.icon.as_ref() {
            let icon = gtk::Image::from_icon_name(icon_name);
            icon.set_pixel_size(16);
            icon.add_css_class("unixnotis-stat-icon");
            header.append(&icon);
        }

        let title = gtk::Label::new(Some(&config.label));
        title.add_css_class("unixnotis-stat-title");
        title.set_xalign(0.0);
        header.append(&title);

        let value_label = gtk::Label::new(Some("n/a"));
        value_label.add_css_class("unixnotis-stat-value");
        value_label.set_xalign(0.0);
        value_label.set_width_chars(12);

        card.append(&header);
        card.append(&value_label);

        let builtin = config
            .cmd
            .as_ref()
            .and_then(|cmd| BuiltinStat::from_command(cmd));

        Self {
            config,
            root: card,
            value_label,
            builtin: RefCell::new(builtin),
            inflight: Rc::new(Cell::new(false)),
            last_value: Rc::new(RefCell::new(None)),
        }
    }

    fn refresh(&self) {
        if !self.root.is_visible() {
            return;
        }
        debug::log(PanelDebugLevel::Verbose, || {
            format!("stat refresh: {}", self.config.label)
        });
        if let Some(builtin) = self.builtin.borrow_mut().as_mut() {
            let value = builtin.read().unwrap_or_else(|| "n/a".to_string());
            self.apply_value(&value);
            return;
        }

        let Some(cmd) = self.config.cmd.as_ref() else {
            self.apply_value("n/a");
            return;
        };
        if self.inflight.get() {
            return;
        }
        self.inflight.set(true);
        let cmd = cmd.clone();
        let rx = run_command_capture_async(&cmd);
        let label = self.value_label.clone();
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
                    warn!(?cmd, ?err, "stat command failed");
                    apply_cached_value(&label, &last_value);
                    return;
                }
            };
            if !output.status.success() {
                warn!(?cmd, "stat command failed");
                apply_cached_value(&label, &last_value);
                return;
            }
            let stdout = String::from_utf8_lossy(&output.stdout);
            let value = stdout.trim();
            if value.is_empty() {
                apply_cached_value(&label, &last_value);
            } else {
                label.set_text(value);
                *last_value.borrow_mut() = Some(value.to_string());
            }
        });
    }

    fn apply_value(&self, value: &str) {
        if self.last_value.borrow().as_deref() == Some(value) {
            return;
        }
        self.value_label.set_text(value);
        *self.last_value.borrow_mut() = Some(value.to_string());
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
