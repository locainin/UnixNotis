//! Panel visibility and input handling for `UiState`
//!
//! Encapsulates open/close requests, focus behavior, and click-outside rules

use std::sync::atomic::Ordering;

use gtk::gdk;
use gtk::prelude::*;
use tracing::debug;
use unixnotis_core::{PanelAction, PanelDebugLevel, PanelRequest};

use crate::control::UiCommand;
use crate::diagnostics::{panel_debug as debug, performance as perf_probe};

use crate::ui::{try_send_command, UiState};

impl UiState {
    pub(in crate::ui) fn apply_panel_request(&mut self, request: PanelRequest) {
        let requested_visibility = panel_visibility_for_action(self.panel_visible, request.action);
        // Request-driven changes always flow through set_visible for consistent side effects
        match request.action {
            PanelAction::Open => {
                debug::set_level(PanelDebugLevel::Off);
            }
            PanelAction::Close => {
                debug::set_level(PanelDebugLevel::Off);
            }
            PanelAction::Toggle => {
                if !self.panel_visible {
                    debug::set_level(PanelDebugLevel::Off);
                }
            }
        }
        self.set_visible(requested_visibility);

        if request.debug != PanelDebugLevel::Off {
            // Debug level overrides apply immediately when requested via control plane
            debug::set_level(request.debug);
            self.log_debug(PanelDebugLevel::Info, || {
                format!("debug mode enabled: {:?}", request.debug)
            });
        }
    }

    fn set_visible(&mut self, visible: bool) {
        self.panel_visible = visible;
        self.panel_visible_flag.store(visible, Ordering::SeqCst);
        debug!(visible, "panel visibility updated");
        self.log_debug(PanelDebugLevel::Info, || {
            format!("panel visibility set to {visible}")
        });
        if visible {
            // Start a new probe window each time panel becomes visible
            perf_probe::on_panel_open(self.panel_visible_flag.clone());
            // Activate watches so widgets only poll while the panel is open
            if let Some(volume) = self.volume.as_ref() {
                volume.set_watch_active(true);
            }
            if let Some(brightness) = self.brightness.as_ref() {
                brightness.set_watch_active(true);
            }
            if let Some(toggles) = self.toggles.as_ref() {
                toggles.set_watch_active(true);
            }
            self.set_widgets_collapsed(self.widgets_collapsed);
            // Pull focus so keyboard navigation starts inside the panel
            self.panel.root.grab_focus();
            if let Some(handle) = self.media_handle.as_ref() {
                // Media refresh is deferred until the panel is visible
                handle.refresh();
            }
            // Apply any pending media state that accumulated while the panel was hidden
            if self.pending_media_cleared {
                if let Some(widget) = self.media.as_mut() {
                    widget.clear();
                }
                self.pending_media_cleared = false;
            }
            if let Some(infos) = self.pending_media.take() {
                if let Some(widget) = self.media.as_mut() {
                    widget.update(&infos);
                }
            }
            // Flush deferred list rebuilds once to avoid repeated background work
            if self.list_needs_rebuild() {
                // Apply any deferred list rebuilds once the panel becomes visible
                self.list.flush_rebuild();
            }
            // Resolve work-area margins before showing the window to avoid a layout shift
            // This prevents a first-frame resize when Hyprland publishes margins after open
            // Only hit the compositor once per open when the cache is empty
            // Keeps open latency stable while avoiding repeated IPC work
            if self.config.panel.respect_work_area && self.work_area.is_none() {
                self.work_area = crate::ui::hyprland::reserved_work_area_sync(
                    self.config.panel.output.as_deref(),
                );
                crate::ui::panel::geometry::apply_panel_config(
                    &self.panel,
                    &self.config,
                    self.work_area,
                );
            }
            // Only show the window after geometry is correct to avoid visible jitter
            self.panel.window.set_visible(true);
            // Refresh counts after pending updates land so header stays accurate
            self.refresh_counts();
            // Run the first widget pass after the window is visible
            // This avoids leaving plugin-backed stats on the n/a placeholder until a later tick
            self.refresh_widgets(true);
            self.start_refresh_timer();
            let width = self.panel.window.width();
            let height = self.panel.window.height();
            let message = format!("panel allocated size: {width}x{height}");
            self.log_debug(PanelDebugLevel::Verbose, move || message);
        } else {
            // Emit a final snapshot before timers and watches are torn down
            perf_probe::on_panel_close();
            // Hide first so any teardown work does not trigger visible reflow
            self.panel.window.set_visible(false);
            // Reset transient search UI so each open starts from the full notification list
            crate::ui::panel::header::search::set_search_open(
                &self.panel.header.actions.search_toggle,
                &self.panel.header.search.revealer,
                &self.panel.header.search.entry,
                self.search_toggle_guard.as_ref(),
                false,
            );
            // Disable watch-based polling when hidden to reduce background load
            if let Some(volume) = self.volume.as_ref() {
                volume.set_watch_active(false);
            }
            if let Some(brightness) = self.brightness.as_ref() {
                brightness.set_watch_active(false);
            }
            if let Some(toggles) = self.toggles.as_ref() {
                toggles.set_watch_active(false);
            }
            self.stop_refresh_timer();
            debug::set_level(PanelDebugLevel::Off);
        }
    }

    pub(in crate::ui) fn close_if_click_outside(&self) {
        if !self.panel_visible {
            return;
        }
        if !self.is_click_outside_panel() {
            self.log_debug(PanelDebugLevel::Verbose, || {
                "click outside ignored (pointer inside panel)".to_string()
            });
            return;
        }
        // Close requests go through the daemon to keep control state consistent
        self.log_debug(PanelDebugLevel::Info, || {
            "click outside detected; requesting close".to_string()
        });
        // Non-blocking send avoids freezing the click handler
        try_send_command(&self.command_tx, UiCommand::ClosePanel);
    }

    pub(in crate::ui) fn log_debug(
        &self,
        level: PanelDebugLevel,
        message: impl FnOnce() -> String,
    ) {
        debug::log(level, message);
    }

    fn is_click_outside_panel(&self) -> bool {
        // Hyprland focus changes can be hover-driven; only close when a mouse button is down
        let Some(display) = gdk::Display::default() else {
            self.log_debug(PanelDebugLevel::Verbose, || {
                "click outside check skipped (no display)".to_string()
            });
            return false;
        };
        let Some(seat) = display.default_seat() else {
            self.log_debug(PanelDebugLevel::Verbose, || {
                "click outside check skipped (no seat)".to_string()
            });
            return false;
        };
        let Some(pointer) = seat.pointer() else {
            self.log_debug(PanelDebugLevel::Verbose, || {
                "click outside check skipped (no pointer)".to_string()
            });
            return false;
        };
        let modifiers = pointer.modifier_state();
        let click_active = modifiers.contains(gdk::ModifierType::BUTTON1_MASK)
            || modifiers.contains(gdk::ModifierType::BUTTON2_MASK)
            || modifiers.contains(gdk::ModifierType::BUTTON3_MASK);
        if !click_active {
            self.log_debug(PanelDebugLevel::Verbose, || {
                "click outside check skipped (no button pressed)".to_string()
            });
            return false;
        }
        let (surface, _, _) = pointer.surface_at_position();
        let panel_surface = self.panel.window.surface();
        if let (Some(surface), Some(panel_surface)) = (surface, panel_surface) {
            if surface == panel_surface {
                self.log_debug(PanelDebugLevel::Verbose, || {
                    "click outside check ignored (surface matches panel)".to_string()
                });
                return false;
            }
        }
        true
    }
}

const fn panel_visibility_for_action(current: bool, action: PanelAction) -> bool {
    match action {
        PanelAction::Open => true,
        PanelAction::Close => false,
        PanelAction::Toggle => !current,
    }
}

#[cfg(test)]
#[path = "tests/visibility.rs"]
mod tests;
