//! Toggle widgets and state synchronization logic.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use gtk::prelude::*;
use gtk::{glib, Align};
use tracing::warn;
use unixnotis_core::{PanelDebugLevel, ToggleWidgetConfig};

use super::util::{
    run_command, run_command_capture_status_async, start_command_watch, CommandWatch,
};
use crate::debug;

pub struct ToggleGrid {
    root: gtk::FlowBox,
    items: Vec<ToggleItem>,
}

struct ToggleItem {
    config: ToggleWidgetConfig,
    button: gtk::ToggleButton,
    guard: Rc<Cell<bool>>,
    refresh_gen: Arc<AtomicU64>,
    watch_handle: Rc<RefCell<Option<CommandWatch>>>,
}

impl ToggleGrid {
    pub fn new(configs: &[ToggleWidgetConfig]) -> Option<Self> {
        let mut items = Vec::new();
        for config in configs {
            if !config.enabled {
                continue;
            }
            items.push(ToggleItem::new(config.clone()));
        }
        if items.is_empty() {
            return None;
        }

        let root = gtk::FlowBox::new();
        root.add_css_class("unixnotis-toggle-grid");
        root.set_selection_mode(gtk::SelectionMode::None);
        root.set_max_children_per_line(4);
        root.set_min_children_per_line(4);
        root.set_row_spacing(8);
        root.set_column_spacing(8);
        root.set_halign(Align::Fill);
        root.set_hexpand(true);

        for item in &items {
            root.insert(&item.button, -1);
        }

        Some(Self { root, items })
    }

    pub fn root(&self) -> &gtk::FlowBox {
        &self.root
    }

    pub fn refresh(&self) {
        for item in &self.items {
            if item.needs_polling() {
                item.refresh();
            }
        }
    }

    pub fn needs_polling(&self) -> bool {
        self.items.iter().any(|item| item.needs_polling())
    }

    pub fn set_watch_active(&self, active: bool) {
        for item in &self.items {
            item.set_watch_active(active);
        }
    }
}

impl ToggleItem {
    fn new(config: ToggleWidgetConfig) -> Self {
        let guard = Rc::new(Cell::new(false));
        let refresh_gen = Arc::new(AtomicU64::new(0));
        let button = gtk::ToggleButton::new();
        button.add_css_class("unixnotis-toggle");
        button.set_focusable(false);

        let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        content.set_halign(Align::Center);
        content.set_valign(Align::Center);
        content.add_css_class("unixnotis-toggle-content");

        let icon = gtk::Image::from_icon_name(&config.icon);
        icon.add_css_class("unixnotis-toggle-icon");
        let label = gtk::Label::new(Some(&config.label));
        label.add_css_class("unixnotis-toggle-label");
        label.set_xalign(0.0);
        label.set_wrap(false);

        content.append(&icon);
        content.append(&label);
        button.set_child(Some(&content));

        let guard_clone = guard.clone();
        let state_cmd = config.state_cmd.clone();
        let on_cmd = config.on_cmd.clone();
        let off_cmd = config.off_cmd.clone();
        let refresh_gen_for_toggle = refresh_gen.clone();
        let label = config.label.clone();
        button.connect_toggled(move |button| {
            if guard_clone.get() {
                return;
            }
            debug::log(PanelDebugLevel::Info, || {
                format!("toggle '{}' set to {}", label, button.is_active())
            });
            let command = if button.is_active() {
                on_cmd.as_ref()
            } else {
                off_cmd.as_ref()
            };
            if let Some(cmd) = command {
                run_command(cmd);
            }
            if let Some(state_cmd) = state_cmd.clone() {
                let guard = guard_clone.clone();
                let refresh_gen = refresh_gen_for_toggle.clone();
                let button = button.clone();
                glib::timeout_add_local(std::time::Duration::from_millis(160), move || {
                    refresh_toggle_state(&state_cmd, &button, &guard, &refresh_gen);
                    glib::ControlFlow::Break
                });
            }
        });

        let item = Self {
            config,
            button,
            guard,
            refresh_gen,
            watch_handle: Rc::new(RefCell::new(None)),
        };
        item.refresh();
        item
    }

    fn refresh(&self) {
        if let Some(state_cmd) = self.config.state_cmd.as_ref() {
            refresh_toggle_state(state_cmd, &self.button, &self.guard, &self.refresh_gen);
        }
    }

    fn needs_polling(&self) -> bool {
        self.watch_handle.borrow().is_none()
    }

    fn set_watch_active(&self, active: bool) {
        if self.config.watch_cmd.is_none() || self.config.state_cmd.is_none() {
            return;
        }
        let mut handle = self.watch_handle.borrow_mut();
        if active {
            if handle.is_none() {
                debug::log(PanelDebugLevel::Info, || {
                    format!("toggle watch enabled: {}", self.config.label)
                });
                *handle = self.start_watch();
            }
        } else {
            if handle.is_some() {
                debug::log(PanelDebugLevel::Info, || {
                    format!("toggle watch disabled: {}", self.config.label)
                });
            }
            handle.take();
        }
    }

    fn start_watch(&self) -> Option<CommandWatch> {
        let watch_cmd = self.config.watch_cmd.as_ref()?;
        let state_cmd = self.config.state_cmd.as_ref()?.clone();
        let button = self.button.clone();
        let guard = self.guard.clone();
        let refresh_gen = self.refresh_gen.clone();
        start_command_watch(watch_cmd, move || {
            refresh_toggle_state(&state_cmd, &button, &guard, &refresh_gen);
        })
    }
}

fn refresh_toggle_state(
    cmd: &str,
    button: &gtk::ToggleButton,
    guard: &Rc<Cell<bool>>,
    refresh_gen: &Arc<AtomicU64>,
) {
    let cmd = cmd.to_string();
    let gen = refresh_gen.fetch_add(1, Ordering::Relaxed) + 1;
    let rx = run_command_capture_status_async(&cmd);
    let button = button.clone();
    let guard = guard.clone();
    let refresh_gen = Arc::clone(refresh_gen);
    glib::MainContext::default().spawn_local(async move {
        let output = match rx.recv().await {
            Ok(output) => output,
            Err(_) => return,
        };
        if refresh_gen.load(Ordering::Relaxed) != gen {
            return;
        }
        let output = match output {
            Ok(output) => output,
            Err(err) => {
                warn!(?cmd, ?err, "toggle state command failed");
                return;
            }
        };
        let success = output.status.success();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let active = if stdout.trim().is_empty() {
            success
        } else {
            parse_toggle_state(&stdout)
        };
        if button.is_active() != active {
            guard.set(true);
            button.set_active(active);
            guard.set(false);
        }
    });
}

fn parse_toggle_state(output: &str) -> bool {
    for line in output.lines() {
        let lower = line.trim().to_ascii_lowercase();
        if lower.contains("powered") || lower.contains("powerstate") {
            if lower.contains("no")
                || lower.contains("off")
                || lower.contains("false")
                || lower.contains("disabled")
            {
                return false;
            }
            if lower.contains("yes")
                || lower.contains("on")
                || lower.contains("true")
                || lower.contains("enabled")
            {
                return true;
            }
        }
    }

    let value = output.trim().to_ascii_lowercase();
    if matches!(
        value.as_str(),
        "1" | "on" | "yes" | "true" | "enabled" | "up"
    ) {
        return true;
    }
    value
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .any(|token| matches!(token, "on" | "yes" | "true" | "enabled" | "up"))
}
