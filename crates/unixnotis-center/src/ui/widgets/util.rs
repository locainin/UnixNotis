//! Shared widget helpers and command plumbing.

#[path = "command_utils.rs"]
mod command_utils;
#[path = "watch_utils.rs"]
mod watch_utils;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{glib, Align};
use tracing::warn;
use unixnotis_core::SliderWidgetConfig;

pub(super) use command_utils::{
    run_command, run_command_capture_async, run_command_capture_status_async,
};
pub(super) use watch_utils::{start_command_watch, CommandWatch};

pub struct CommandSlider {
    pub root: gtk::Box,
    scale: gtk::Scale,
    value_label: gtk::Label,
    icon_button: gtk::Button,
    icon_name: String,
    icon_muted: Option<String>,
    config: SliderWidgetConfig,
    updating: Rc<Cell<bool>>,
    refresh_gen: Arc<AtomicU64>,
    watch_handle: RefCell<Option<CommandWatch>>,
}

impl CommandSlider {
    pub fn new(config: SliderWidgetConfig, extra_class: &str) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        root.add_css_class("unixnotis-quick-slider");
        root.add_css_class(extra_class);

        let icon_button = gtk::Button::from_icon_name(&config.icon);
        icon_button.add_css_class("unixnotis-quick-slider-icon");
        icon_button.set_valign(Align::Center);
        icon_button.set_halign(Align::Center);

        let scale = gtk::Scale::with_range(
            gtk::Orientation::Horizontal,
            config.min,
            config.max,
            config.step,
        );
        scale.set_draw_value(false);
        scale.set_hexpand(true);
        scale.set_vexpand(false);
        scale.set_valign(Align::Center);
        // Ensure GTK gets a non-negative minimum size to avoid layout warnings.
        scale.set_size_request(180, 24);
        scale.set_width_request(180);
        scale.set_height_request(24);
        scale.add_css_class("unixnotis-quick-slider-scale");

        let value_label = gtk::Label::new(Some("0%"));
        value_label.add_css_class("unixnotis-quick-slider-value");
        value_label.set_valign(Align::Center);

        root.append(&icon_button);
        root.append(&scale);
        root.append(&value_label);

        let updating = Rc::new(Cell::new(false));
        let pending = Rc::new(RefCell::new(None));
        let pending_value = Rc::new(Cell::new(None));
        let refresh_gen = Arc::new(AtomicU64::new(0));
        let icon_name = config.icon.clone();
        let icon_muted = config.icon_muted.clone();
        let min = config.min;
        let max = config.max;

        if let Some(toggle_cmd) = config.toggle_cmd.as_ref() {
            let cmd = toggle_cmd.clone();
            let refresh_cmd = config.get_cmd.clone();
            let refresh_scale = scale.clone();
            let refresh_label = value_label.clone();
            let refresh_icon = icon_button.clone();
            let refresh_updating = updating.clone();
            let refresh_gen = refresh_gen.clone();
            let refresh_icon_name = icon_name.clone();
            let refresh_icon_muted = icon_muted.clone();
            icon_button.connect_clicked(move |_| {
                run_command(&cmd);
                let refresh_cmd = refresh_cmd.clone();
                let refresh_scale = refresh_scale.clone();
                let refresh_label = refresh_label.clone();
                let refresh_icon = refresh_icon.clone();
                let refresh_updating = refresh_updating.clone();
                let refresh_gen = refresh_gen.clone();
                let refresh_icon_name = refresh_icon_name.clone();
                let refresh_icon_muted = refresh_icon_muted.clone();
                glib::timeout_add_local(Duration::from_millis(160), move || {
                    refresh_inner(
                        refresh_cmd.clone(),
                        min,
                        max,
                        refresh_scale.clone(),
                        refresh_label.clone(),
                        refresh_icon.clone(),
                        refresh_updating.clone(),
                        refresh_gen.clone(),
                        refresh_icon_name.clone(),
                        refresh_icon_muted.clone(),
                    );
                    glib::ControlFlow::Break
                });
            });
        } else {
            icon_button.set_sensitive(false);
        }

        let set_cmd = config.set_cmd.clone();
        let updating_guard = updating.clone();
        let pending_guard = pending.clone();
        let pending_value_guard = pending_value.clone();
        let label_clone = value_label.clone();
        scale.connect_value_changed(move |scale| {
            if updating_guard.get() {
                return;
            }
            let value = scale.value();
            label_clone.set_text(&format_value(value));
            schedule_command(
                pending_guard.clone(),
                pending_value_guard.clone(),
                set_cmd.clone(),
                value,
            );
        });

        Self {
            root,
            scale,
            value_label,
            icon_button,
            icon_name,
            icon_muted,
            config,
            updating,
            refresh_gen,
            watch_handle: RefCell::new(None),
        }
    }

    pub fn refresh(&self) {
        refresh_inner(
            self.config.get_cmd.clone(),
            self.config.min,
            self.config.max,
            self.scale.clone(),
            self.value_label.clone(),
            self.icon_button.clone(),
            self.updating.clone(),
            self.refresh_gen.clone(),
            self.icon_name.clone(),
            self.icon_muted.clone(),
        );
    }

    pub fn needs_polling(&self) -> bool {
        self.watch_handle.borrow().is_none()
    }

    pub fn set_watch_active(&self, active: bool) {
        if self.config.watch_cmd.is_none() {
            return;
        }
        let mut handle = self.watch_handle.borrow_mut();
        if active {
            if handle.is_none() {
                *handle = self.start_watch();
            }
        } else {
            handle.take();
        }
    }

    fn start_watch(&self) -> Option<CommandWatch> {
        let cmd = self.config.watch_cmd.as_ref()?;
        let refresh_cmd = self.config.get_cmd.clone();
        let refresh_scale = self.scale.clone();
        let refresh_label = self.value_label.clone();
        let refresh_icon = self.icon_button.clone();
        let refresh_updating = self.updating.clone();
        let refresh_gen = self.refresh_gen.clone();
        let refresh_icon_name = self.icon_name.clone();
        let refresh_icon_muted = self.icon_muted.clone();
        let min = self.config.min;
        let max = self.config.max;
        start_command_watch(cmd, move || {
            refresh_inner(
                refresh_cmd.clone(),
                min,
                max,
                refresh_scale.clone(),
                refresh_label.clone(),
                refresh_icon.clone(),
                refresh_updating.clone(),
                refresh_gen.clone(),
                refresh_icon_name.clone(),
                refresh_icon_muted.clone(),
            );
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn refresh_inner(
    cmd: String,
    min: f64,
    max: f64,
    scale: gtk::Scale,
    label: gtk::Label,
    icon_button: gtk::Button,
    updating: Rc<Cell<bool>>,
    refresh_gen: Arc<AtomicU64>,
    icon_name: String,
    icon_muted: Option<String>,
) {
    let gen = refresh_gen.fetch_add(1, Ordering::Relaxed) + 1;

    let rx = run_command_capture_status_async(&cmd);
    let refresh_gen = refresh_gen.clone();
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
                warn!(?err, "slider command failed");
                return;
            }
        };
        if !output.status.success() {
            warn!(?cmd, "slider command returned error");
            return;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let value = match parse_numeric(&stdout, min, max) {
            Some(value) => value,
            None => return,
        };
        let muted = parse_muted(&stdout);

        updating.set(true);
        scale.set_value(value);
        label.set_text(&format_value(value));
        if let Some(icon_muted) = icon_muted.as_ref() {
            let icon = if muted { icon_muted } else { &icon_name };
            icon_button.set_icon_name(icon);
        }
        updating.set(false);
    });
}

fn schedule_command(
    pending: Rc<RefCell<Option<glib::SourceId>>>,
    pending_value: Rc<Cell<Option<f64>>>,
    cmd_template: String,
    value: f64,
) {
    pending_value.set(Some(value));
    if pending.borrow().is_some() {
        return;
    }

    let pending_guard = pending.clone();
    let pending_value = pending_value.clone();
    let id = glib::timeout_add_local(Duration::from_millis(120), move || {
        let value = pending_value.replace(None);
        let _ = pending_guard.borrow_mut().take();
        if let Some(value) = value {
            let formatted = cmd_template.replace("{value}", &format!("{value:.0}"));
            run_command(&formatted);
        }
        glib::ControlFlow::Break
    });
    *pending.borrow_mut() = Some(id);
}

fn parse_numeric(text: &str, min: f64, max: f64) -> Option<f64> {
    #[derive(Clone)]
    struct Token {
        value: f64,
        raw: String,
        percent: bool,
    }

    let mut current = String::new();
    let mut tokens: Vec<Token> = Vec::new();

    for ch in text.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            current.push(ch);
            continue;
        }
        if !current.is_empty() {
            if let Ok(value) = current.parse::<f64>() {
                let percent = ch == '%';
                tokens.push(Token {
                    value,
                    raw: current.clone(),
                    percent,
                });
            }
            current.clear();
        }
    }

    if !current.is_empty() {
        if let Ok(value) = current.parse::<f64>() {
            tokens.push(Token {
                value,
                raw: current.clone(),
                percent: false,
            });
        }
    }

    let token = tokens
        .iter()
        .rev()
        .find(|token| token.percent)
        .or_else(|| tokens.last())?;
    let mut value = token.value;

    // Heuristic: If the token contains a decimal point, it's likely a normalized ratio (0.0-1.0+)
    // from tools like wpctl. Scale to percentage.
    if token.raw.contains('.') && value <= 5.0 {
        value *= 100.0;
    }

    Some(value.clamp(min, max))
}

fn parse_muted(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("muted") || lower.contains("mute: yes")
}

fn format_value(value: f64) -> String {
    format!("{value:.0}%")
}
