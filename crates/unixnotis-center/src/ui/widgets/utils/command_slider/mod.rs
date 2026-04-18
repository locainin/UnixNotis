//! Command-backed slider widget and refresh wiring

mod refresh;
mod schedule;
#[cfg(test)]
mod tests;
mod value;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::Align;
use unixnotis_core::PanelDebugLevel;
use unixnotis_core::SliderWidgetConfig;

use self::refresh::{request_refresh, SliderRefreshGate, SliderRefreshMeta, SliderRefreshState};
use self::schedule::schedule_command;
use super::slider_icons::resolve_slider_icon_name;
use super::slider_parse::format_value;
use super::{run_action_command_with_completion, start_command_watch, CommandWatch};
use crate::debug;

pub struct CommandSlider {
    // Root widget embedded by higher-level widget wrappers
    pub root: gtk::Box,
    // Interactive value control
    scale: gtk::Scale,
    // Text label for the current slider value
    value_label: gtk::Label,
    // Icon image is reused by both clickable and static slider variants
    icon_image: gtk::Image,
    // Default icon shown when slider is not muted
    icon_name: String,
    // Optional icon variant for muted state
    icon_muted: Option<String>,
    // Config is retained for refresh and watch lifecycle operations
    config: SliderWidgetConfig,
    // Guard blocks recursive value-changed signals during internal updates
    updating: Rc<Cell<bool>>,
    // Generation token avoids stale async refresh races
    refresh_gen: Rc<Cell<u64>>,
    // Local gate keeps refresh bursts bounded
    refresh_gate: SliderRefreshGate,
    // Optional watch command handle for event-driven refresh
    watch_handle: RefCell<Option<CommandWatch>>,
}

impl CommandSlider {
    pub fn new(config: SliderWidgetConfig, extra_class: &str) -> Self {
        // Root combines base style with caller-provided variant class
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        root.add_css_class("unixnotis-quick-slider");
        root.add_css_class(extra_class);

        // Resolve icon upfront so themes without exact names still render valid glyphs
        let icon_name = resolve_slider_icon_name(&config.label, &config.icon);
        let icon_image = gtk::Image::from_icon_name(&icon_name);
        icon_image.set_valign(Align::Center);
        icon_image.set_halign(Align::Center);

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
        // One size request is enough here and keeps the layout less rigid
        scale.set_size_request(180, 24);
        scale.add_css_class("unixnotis-quick-slider-scale");

        // Keep the first frame neutral so generic sliders do not imply a percent value
        let value_label = gtk::Label::new(Some("----"));
        value_label.add_css_class("unixnotis-quick-slider-value");
        value_label.set_valign(Align::Center);
        value_label.set_xalign(1.0);
        value_label.set_width_chars(4);
        root.append(&scale);
        root.append(&value_label);

        let updating = Rc::new(Cell::new(false));
        // Debounce state coalesces slider drags into fewer set_cmd executions
        let pending = Rc::new(RefCell::new(None));
        let pending_value = Rc::new(Cell::new(None));
        // Refresh generation only lives on the GTK main loop
        // Rc<Cell<_>> keeps this path lighter than Arc<AtomicU64>
        let refresh_gen = Rc::new(Cell::new(0_u64));
        let refresh_gate = SliderRefreshGate::new();
        let icon_muted = config
            .icon_muted
            .as_deref()
            .map(|name| resolve_slider_icon_name(&config.label, name));
        let min = config.min;
        let max = config.max;
        let step = config.step;
        let parse_mode = config.parse_mode;
        let refresh_cmd = config.get_cmd.clone();
        let refresh_meta = SliderRefreshMeta {
            updating: updating.clone(),
            refresh_gen: refresh_gen.clone(),
            icon_name: icon_name.clone(),
            icon_muted: icon_muted.clone(),
            gate: refresh_gate.clone(),
        };

        if let Some(toggle_cmd) = config.toggle_cmd.as_ref() {
            let cmd = toggle_cmd.clone();
            let icon_button = build_icon_shell(&icon_image, true);
            root.prepend(&icon_button);

            let scale_weak = scale.downgrade();
            let label_weak = value_label.downgrade();
            let icon_weak = icon_image.downgrade();
            let refresh_cmd = refresh_cmd.clone();
            let refresh_meta = refresh_meta.clone();
            icon_button.connect_clicked(move |_| {
                let scale_weak = scale_weak.clone();
                let label_weak = label_weak.clone();
                let icon_weak = icon_weak.clone();
                let refresh_cmd = refresh_cmd.clone();
                let refresh_meta = refresh_meta.clone();
                run_action_command_with_completion(
                    cmd.clone(),
                    "slider toggle action",
                    move |failed| {
                        if failed {
                            // Failed actions still need one refresh so UI snaps back to real state
                            debug::log(PanelDebugLevel::Warn, || {
                                format!(
                                    "slider toggle action failed; forcing refresh cmd=\"{}\"",
                                    refresh_cmd
                                )
                            });
                        }

                        let Some(refresh) = build_refresh_state_from_weak(
                            &scale_weak,
                            &label_weak,
                            &icon_weak,
                            &refresh_meta,
                        ) else {
                            return;
                        };
                        request_refresh(refresh_cmd.clone(), min, max, step, parse_mode, refresh);
                    },
                );
            });
        } else {
            // Static sliders still use the same shell widget as clickable sliders
            // This keeps default template alignment stable between volume and brightness rows
            let icon_shell = build_icon_shell(&icon_image, false);
            root.prepend(&icon_shell);
        }

        let set_cmd = config.set_cmd.clone();
        let refresh_cmd_for_set = refresh_cmd.clone();
        let refresh_meta_for_set = refresh_meta.clone();
        let updating_guard = updating.clone();
        let pending_guard = pending.clone();
        let pending_value_guard = pending_value.clone();
        let scale_weak = scale.downgrade();
        let label_weak = value_label.downgrade();
        let icon_weak = icon_image.downgrade();
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
                Rc::new({
                    let scale_weak = scale_weak.clone();
                    let label_weak = label_weak.clone();
                    let icon_weak = icon_weak.clone();
                    let refresh_cmd = refresh_cmd_for_set.clone();
                    let refresh_meta = refresh_meta_for_set.clone();
                    move |failed| {
                        if !failed {
                            return;
                        }
                        // Failed set actions should reconcile quickly instead of waiting for polling
                        debug::log(PanelDebugLevel::Warn, || {
                            format!(
                                "slider set action failed; forcing refresh cmd=\"{}\"",
                                refresh_cmd
                            )
                        });
                        let Some(refresh) = build_refresh_state_from_weak(
                            &scale_weak,
                            &label_weak,
                            &icon_weak,
                            &refresh_meta,
                        ) else {
                            return;
                        };
                        request_refresh(refresh_cmd.clone(), min, max, step, parse_mode, refresh);
                    }
                }),
            );
        });

        Self {
            root,
            scale,
            value_label,
            icon_image,
            icon_name,
            icon_muted,
            config,
            updating,
            refresh_gen,
            refresh_gate,
            watch_handle: RefCell::new(None),
        }
    }

    pub fn refresh(&self) {
        // Public refresh path delegates to shared async fetch routine
        request_refresh(
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
            request_refresh(
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
            icon_image: self.icon_image.clone(),
            updating: self.updating.clone(),
            refresh_gen: self.refresh_gen.clone(),
            icon_name: self.icon_name.clone(),
            icon_muted: self.icon_muted.clone(),
            gate: self.refresh_gate.clone(),
        }
    }
}

fn build_icon_shell(icon_image: &gtk::Image, interactive: bool) -> gtk::Button {
    let button = gtk::Button::new();
    button.set_child(Some(icon_image));
    button.add_css_class("unixnotis-quick-slider-icon");
    button.set_valign(Align::Center);
    button.set_halign(Align::Center);
    // Only clickable sliders should take pointer or focus events
    // Static shells still use the same GTK node so row metrics stay aligned
    button.set_focusable(interactive);
    button.set_can_target(interactive);
    button
}

fn build_refresh_state_from_weak(
    scale: &glib::WeakRef<gtk::Scale>,
    label: &glib::WeakRef<gtk::Label>,
    icon_image: &glib::WeakRef<gtk::Image>,
    refresh_meta: &SliderRefreshMeta,
) -> Option<SliderRefreshState> {
    // Widget teardown is normal, so stale async completions just stop here
    let scale = scale.upgrade()?;
    let label = label.upgrade()?;
    let icon_image = icon_image.upgrade()?;
    Some(SliderRefreshState {
        scale,
        label,
        icon_image,
        updating: refresh_meta.updating.clone(),
        refresh_gen: refresh_meta.refresh_gen.clone(),
        icon_name: refresh_meta.icon_name.clone(),
        icon_muted: refresh_meta.icon_muted.clone(),
        gate: refresh_meta.gate.clone(),
    })
}
