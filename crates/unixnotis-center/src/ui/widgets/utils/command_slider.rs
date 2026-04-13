//! Command-backed slider widget and refresh wiring

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use glib::clone;
use gtk::prelude::*;
use gtk::Align;
use tracing::warn;
use unixnotis_core::{util, NumericParseMode, PanelDebugLevel, SliderWidgetConfig};

use super::slider_icons::resolve_slider_icon_name;
use super::slider_parse::{format_value, parse_muted, parse_numeric};
use super::{run_command, run_command_capture_status_async, start_command_watch, CommandWatch};
use crate::debug;

pub struct CommandSlider {
    // Root widget embedded by higher-level widget wrappers
    pub root: gtk::Box,
    // Interactive value control
    scale: gtk::Scale,
    // Human-readable percentage label
    value_label: gtk::Label,
    // Icon button used for mute/toggle actions when configured
    icon_button: gtk::Button,
    // Default icon shown when slider is not muted
    icon_name: String,
    // Optional icon variant for muted state
    icon_muted: Option<String>,
    // Config is retained for refresh and watch lifecycle operations
    config: SliderWidgetConfig,
    // Guard blocks recursive value-changed signals during internal updates
    updating: Rc<Cell<bool>>,
    // Generation token avoids stale async refresh races
    refresh_gen: Arc<AtomicU64>,
    // Optional watch command handle for event-driven refresh
    watch_handle: RefCell<Option<CommandWatch>>,
}

#[derive(Clone)]
struct SliderRefreshState {
    // Slider updated from command output
    scale: gtk::Scale,
    // Label kept in sync with the slider
    label: gtk::Label,
    // Icon button updated after refresh
    icon_button: gtk::Button,
    // Guard stops refresh writes from triggering another set command
    updating: Rc<Cell<bool>>,
    // Generation drops stale async refresh results
    refresh_gen: Arc<AtomicU64>,
    // Normal icon shown when not muted
    icon_name: String,
    // Optional icon used when muted
    icon_muted: Option<String>,
}

#[derive(Clone)]
struct SliderRefreshMeta {
    // Non-widget refresh state that is safe to hold across signal closures
    updating: Rc<Cell<bool>>,
    // Generation drops stale async refresh results
    refresh_gen: Arc<AtomicU64>,
    // Normal icon shown when not muted
    icon_name: String,
    // Optional icon used when muted
    icon_muted: Option<String>,
}

impl CommandSlider {
    pub fn new(config: SliderWidgetConfig, extra_class: &str) -> Self {
        // Root combines base style with caller-provided variant class
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        root.add_css_class("unixnotis-quick-slider");
        root.add_css_class(extra_class);

        // Resolve icon upfront so themes without exact names still render valid glyphs
        let icon_name = resolve_slider_icon_name(&config.label, &config.icon);
        let icon_button = gtk::Button::from_icon_name(&icon_name);
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
        // Ensure GTK gets a non-negative minimum size to avoid layout warnings
        scale.set_size_request(180, 24);
        scale.set_width_request(180);
        scale.set_height_request(24);
        scale.add_css_class("unixnotis-quick-slider-scale");

        let value_label = gtk::Label::new(Some("0%"));
        value_label.add_css_class("unixnotis-quick-slider-value");
        value_label.set_valign(Align::Center);
        value_label.set_xalign(1.0);
        value_label.set_width_chars(4);

        root.append(&icon_button);
        root.append(&scale);
        root.append(&value_label);

        let updating = Rc::new(Cell::new(false));
        // Debounce state coalesces slider drags into fewer set_cmd executions
        let pending = Rc::new(RefCell::new(None));
        let pending_value = Rc::new(Cell::new(None));
        let refresh_gen = Arc::new(AtomicU64::new(0));
        let icon_muted = config
            .icon_muted
            .as_deref()
            .map(|name| resolve_slider_icon_name(&config.label, name));
        let min = config.min;
        let max = config.max;
        let step = config.step;
        let parse_mode = config.parse_mode;

        if let Some(toggle_cmd) = config.toggle_cmd.as_ref() {
            let cmd = toggle_cmd.clone();
            let refresh_cmd = config.get_cmd.clone();
            let refresh_meta = SliderRefreshMeta {
                updating: updating.clone(),
                refresh_gen: refresh_gen.clone(),
                icon_name: icon_name.clone(),
                icon_muted: icon_muted.clone(),
            };
            icon_button.connect_clicked(clone!(
                #[weak]
                icon_button,
                #[weak]
                scale,
                #[weak]
                value_label,
                #[strong]
                cmd,
                #[strong]
                refresh_cmd,
                #[strong]
                refresh_meta,
                move |_| {
                    // Weak widget captures avoid GTK reference cycles here
                    run_command(&cmd);
                    glib::timeout_add_local(
                        Duration::from_millis(160),
                        clone!(
                            #[weak]
                            icon_button,
                            #[weak]
                            scale,
                            #[weak]
                            value_label,
                            #[strong]
                            refresh_cmd,
                            #[strong]
                            refresh_meta,
                            #[upgrade_or]
                            glib::ControlFlow::Break,
                            move || {
                                // Rebuild widget state only after weak refs were upgraded
                                let refresh = SliderRefreshState {
                                    scale: scale.clone(),
                                    label: value_label.clone(),
                                    icon_button: icon_button.clone(),
                                    updating: refresh_meta.updating.clone(),
                                    refresh_gen: refresh_meta.refresh_gen.clone(),
                                    icon_name: refresh_meta.icon_name.clone(),
                                    icon_muted: refresh_meta.icon_muted.clone(),
                                };
                                refresh_inner(
                                    refresh_cmd.clone(),
                                    min,
                                    max,
                                    step,
                                    parse_mode,
                                    refresh,
                                );
                                glib::ControlFlow::Break
                            }
                        ),
                    );
                }
            ));
        } else {
            icon_button.set_sensitive(false);
        }

        let set_cmd = config.set_cmd.clone();
        let updating_guard = updating.clone();
        let pending_guard = pending.clone();
        let pending_value_guard = pending_value.clone();
        let label_clone = value_label.clone();
        scale.connect_value_changed(move |scale| {
            // Skip callback body when value is being updated programmatically
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
                step,
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
        // Public refresh path delegates to shared async fetch routine
        refresh_inner(
            self.config.get_cmd.clone(),
            self.config.min,
            self.config.max,
            self.config.step,
            self.config.parse_mode,
            self.refresh_state(),
        );
    }

    pub fn needs_polling(&self) -> bool {
        let mut handle = self.watch_handle.borrow_mut();
        if let Some(watch) = handle.as_ref() {
            // If the watch command exited, fall back to polling and allow a new watch later
            if !watch.is_active() {
                handle.take();
                return true;
            }
            return false;
        }
        true
    }

    pub fn set_watch_active(&self, active: bool) {
        // Widgets without a watch command rely on polling only
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
        // Watch callbacks reuse polling refresh logic to keep semantics consistent
        let cmd = self.config.watch_cmd.as_ref()?;
        let refresh_cmd = self.config.get_cmd.clone();
        let refresh_state = self.refresh_state();
        let min = self.config.min;
        let max = self.config.max;
        let step = self.config.step;
        let parse_mode = self.config.parse_mode;
        start_command_watch(cmd, move || {
            refresh_inner(
                refresh_cmd.clone(),
                min,
                max,
                step,
                parse_mode,
                refresh_state.clone(),
            );
        })
    }

    fn refresh_state(&self) -> SliderRefreshState {
        // Build one refresh bundle so call sites stay small
        SliderRefreshState {
            scale: self.scale.clone(),
            label: self.value_label.clone(),
            icon_button: self.icon_button.clone(),
            updating: self.updating.clone(),
            refresh_gen: self.refresh_gen.clone(),
            icon_name: self.icon_name.clone(),
            icon_muted: self.icon_muted.clone(),
        }
    }
}

fn refresh_inner(
    cmd: String,
    min: f64,
    max: f64,
    step: f64,
    parse_mode: NumericParseMode,
    refresh: SliderRefreshState,
) {
    // New refresh id makes older async results stale
    let gen = refresh.refresh_gen.fetch_add(1, Ordering::Relaxed) + 1;

    let rx = run_command_capture_status_async(&cmd);
    let refresh_gen = refresh.refresh_gen.clone();
    glib::MainContext::default().spawn_local(async move {
        let output = match rx.recv().await {
            Ok(output) => output,
            Err(_) => return,
        };
        if refresh_gen.load(Ordering::Relaxed) != gen {
            // A newer refresh already started so this result is old
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
        let value = match parse_numeric(&stdout, min, max, parse_mode) {
            Some(value) => value,
            None => {
                let snippet = util::log_snippet(stdout.trim());
                debug::log(PanelDebugLevel::Warn, || {
                    format!("slider parse failed cmd=\"{}\" output=\"{}\"", cmd, snippet)
                });
                return;
            }
        };
        let muted = parse_muted(&stdout);

        let formatted = format_value(value);
        // Skip widget writes when the visible state is already current
        let value_changed = slider_value_changed(refresh.scale.value(), value, step);
        let label_changed = refresh.label.text().as_str() != formatted;
        if value_changed || label_changed {
            refresh.updating.set(true);
            if value_changed {
                refresh.scale.set_value(value);
            }
            if label_changed {
                refresh.label.set_text(&formatted);
            }
            refresh.updating.set(false);
            debug::log(PanelDebugLevel::Verbose, || {
                format!(
                    "slider updated cmd=\"{}\" value={value:.1} muted={muted}",
                    cmd
                )
            });
        }
        if let Some(icon_muted) = refresh.icon_muted.as_ref() {
            // Not every slider has a muted icon pair
            let icon = if muted {
                icon_muted
            } else {
                &refresh.icon_name
            };
            refresh.icon_button.set_icon_name(icon);
        }
    });
}

fn schedule_command(
    pending: Rc<RefCell<Option<glib::SourceId>>>,
    pending_value: Rc<Cell<Option<f64>>>,
    cmd_template: String,
    value: f64,
    step: f64,
) {
    // Latest value wins while debounce timer is active
    pending_value.set(Some(value));
    if pending.borrow().is_some() {
        return;
    }

    let value_text = format_command_value(value, step);
    debug::log(PanelDebugLevel::Verbose, || {
        format!("slider set scheduled value={value_text}")
    });
    let pending_guard = pending.clone();
    let pending_value = pending_value.clone();
    let id = glib::timeout_add_local(Duration::from_millis(120), move || {
        // Drain pending state and execute the most recent queued command
        let value = pending_value.replace(None);
        let _ = pending_guard.borrow_mut().take();
        if let Some(value) = value {
            let formatted = cmd_template.replace("{value}", &format_command_value(value, step));
            run_command(&formatted);
        }
        glib::ControlFlow::Break
    });
    *pending.borrow_mut() = Some(id);
}

fn slider_value_changed(current: f64, next: f64, step: f64) -> bool {
    // Treat values inside half a step as unchanged for UI refresh decisions
    (current - next).abs() > slider_value_tolerance(step)
}

fn slider_value_tolerance(step: f64) -> f64 {
    // Broken or missing step values fall back to a tiny fixed tolerance
    if !step.is_finite() || step <= 0.0 {
        return 1e-6;
    }
    (step * 0.5).max(1e-6)
}

fn format_command_value(value: f64, step: f64) -> String {
    // Match command precision to slider granularity so fractional sliders stay correct
    let precision = slider_step_precision(step);
    let formatted = format!("{value:.precision$}");
    trim_decimal_suffix(formatted)
}

fn slider_step_precision(step: f64) -> usize {
    if !step.is_finite() || step <= 0.0 {
        return 0;
    }

    // Stop once the step looks like a whole number at this precision
    for precision in 0..=6 {
        let factor = 10f64.powi(precision as i32);
        let scaled = step * factor;
        if (scaled.round() - scaled).abs() <= 1e-9 {
            return precision;
        }
    }

    6
}

fn trim_decimal_suffix(mut text: String) -> String {
    // Drop trailing zeroes so commands get `12.5` instead of `12.500`
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::{format_command_value, slider_value_changed, slider_value_tolerance};

    #[test]
    fn format_command_value_keeps_fractional_precision_from_step() {
        assert_eq!(format_command_value(12.5, 0.5), "12.5");
        assert_eq!(format_command_value(12.25, 0.25), "12.25");
        assert_eq!(format_command_value(12.125, 0.125), "12.125");
    }

    #[test]
    fn format_command_value_trims_integer_suffix_when_step_is_whole() {
        assert_eq!(format_command_value(42.0, 1.0), "42");
        assert_eq!(format_command_value(42.0, 10.0), "42");
    }

    #[test]
    fn slider_value_changed_uses_step_sized_tolerance() {
        assert_eq!(slider_value_tolerance(0.1), 0.05);
        assert!(!slider_value_changed(50.0, 50.04, 0.1));
        assert!(slider_value_changed(50.0, 50.06, 0.1));
    }
}
