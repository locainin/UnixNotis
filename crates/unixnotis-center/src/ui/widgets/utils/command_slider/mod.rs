//! Command-backed slider widget and refresh wiring

mod refresh;
mod schedule;
#[cfg(test)]
mod tests;
mod value;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Duration;

use glib::clone;
use gtk::prelude::*;
use gtk::Align;
use unixnotis_core::SliderWidgetConfig;

use self::refresh::{refresh_inner, SliderRefreshMeta, SliderRefreshState};
use self::schedule::schedule_command;
use super::slider_icons::resolve_slider_icon_name;
use super::slider_parse::format_value;
use super::{run_command, start_command_watch, CommandWatch};

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
