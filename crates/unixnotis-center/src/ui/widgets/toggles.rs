//! Toggle widgets and state synchronization logic.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

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

// Staggered delays keep post-action refreshes responsive without continuous polling.
const TOGGLE_REFRESH_DELAYS_MS: &[u64] = &[0, 50, 100, 200, 400, 800];

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
                // The button state reflects user intent; the retries reconcile it with reality.
                let expected = button.is_active();
                schedule_toggle_refresh_with_retry(state_cmd, expected, button, guard, refresh_gen);
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
        let mut handle = self.watch_handle.borrow_mut();
        if let Some(watch) = handle.as_ref() {
            // Drop inactive watches so polling can keep the toggle state in sync.
            if !watch.is_active() {
                handle.take();
                return true;
            }
            return false;
        }
        true
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
    // Generation tokens prevent stale refreshes from overwriting newer state.
    let gen = refresh_gen.fetch_add(1, Ordering::Relaxed) + 1;
    let button = button.clone();
    let guard = guard.clone();
    let refresh_gen = Arc::clone(refresh_gen);
    glib::MainContext::default().spawn_local(async move {
        let Some(active) = fetch_toggle_state(&cmd, true).await else {
            return;
        };
        if refresh_gen.load(Ordering::Relaxed) != gen {
            return;
        }
        if button.is_active() != active {
            guard.set(true);
            button.set_active(active);
            guard.set(false);
        }
    });
}

fn schedule_toggle_refresh_with_retry(
    state_cmd: String,
    expected: bool,
    button: gtk::ToggleButton,
    guard: Rc<Cell<bool>>,
    refresh_gen: Arc<AtomicU64>,
) {
    // Bounded retry keeps the UI honest for slow toggles without long-lived polling.
    let gen = refresh_gen.fetch_add(1, Ordering::Relaxed) + 1;
    // Weak refs prevent the retry task from keeping the widget tree alive.
    let button_weak = button.downgrade();
    let guard_weak = Rc::downgrade(&guard);
    let refresh_gen_weak = Arc::downgrade(&refresh_gen);
    glib::MainContext::default().spawn_local(async move {
        for (attempt, delay_ms) in TOGGLE_REFRESH_DELAYS_MS.iter().enumerate() {
            if *delay_ms > 0 {
                glib::timeout_future(Duration::from_millis(*delay_ms)).await;
            }
            let Some(refresh_gen) = refresh_gen_weak.upgrade() else {
                return;
            };
            if refresh_gen.load(Ordering::Relaxed) != gen {
                return;
            }
            // Limit warnings to the first attempt to avoid log spam during retries.
            let log_failures = attempt == 0;
            let Some(active) = fetch_toggle_state(&state_cmd, log_failures).await else {
                continue;
            };
            if refresh_gen.load(Ordering::Relaxed) != gen {
                return;
            }
            let (Some(button), Some(guard)) = (button_weak.upgrade(), guard_weak.upgrade()) else {
                // Stop retries if the UI has been dropped to avoid needless work.
                return;
            };
            if button.is_active() != active {
                guard.set(true);
                button.set_active(active);
                guard.set(false);
            }
            if active == expected {
                return;
            }
        }
    });
}

async fn fetch_toggle_state(cmd: &str, log_failures: bool) -> Option<bool> {
    let rx = run_command_capture_status_async(cmd);
    let output = match rx.recv().await {
        Ok(output) => output,
        Err(_) => return None,
    };
    let output = match output {
        Ok(output) => output,
        Err(err) => {
            if log_failures {
                warn!(?cmd, ?err, "toggle state command failed");
            }
            return None;
        }
    };
    let success = output.status.success();
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Empty stdout is treated as success/failure status, otherwise parse text.
    let active = if stdout.trim().is_empty() {
        success
    } else {
        parse_toggle_state(&stdout)
    };
    Some(active)
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
    // systemctl is-active returns "active"/"inactive"/"failed".
    if matches!(value.as_str(), "active" | "activated") {
        return true;
    }
    if matches!(value.as_str(), "inactive" | "failed" | "dead") {
        return false;
    }
    if matches!(
        value.as_str(),
        "1" | "on" | "yes" | "true" | "enabled" | "up"
    ) {
        return true;
    }
    value
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .any(|token| matches!(token, "on" | "yes" | "true" | "enabled" | "up" | "active"))
}
