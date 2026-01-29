//! Panel visibility and input handling for `UiState`.
//!
//! Encapsulates open/close requests, focus behavior, and click-outside rules.

use std::sync::atomic::Ordering;

use gtk::gdk;
use gtk::prelude::*;
use tracing::debug;
use unixnotis_core::{PanelAction, PanelDebugLevel, PanelRequest};

use crate::dbus::UiCommand;
use crate::debug;

use super::{try_send_command, UiState};

impl UiState {
    pub(super) fn update_state(&mut self, state: unixnotis_core::ControlState) {
        // Avoid re-entrant DND toggles while applying daemon state.
        self.dnd_guard.set(true);
        self.panel.dnd_toggle.set_active(state.dnd_enabled);
        self.dnd_guard.set(false);
    }

    pub(super) fn refresh_counts(&mut self) {
        if !self.panel_visible {
            // Skip label updates while hidden to avoid unnecessary UI work.
            // Counts are refreshed on the next open to keep the header accurate.
            return;
        }
        // Header count always reflects total active + history entries.
        let total = self.list.total_count();
        if self.last_count == Some(total) {
            return;
        }
        self.last_count = Some(total);
        self.panel.header_count.set_text(&format!("{total}"));
    }

    pub(super) fn apply_panel_request(&mut self, request: PanelRequest) {
        // Request-driven changes always flow through set_visible for consistent side effects.
        match request.action {
            PanelAction::Open => {
                debug::set_level(PanelDebugLevel::Off);
                self.set_visible(true);
            }
            PanelAction::Close => {
                debug::set_level(PanelDebugLevel::Off);
                self.set_visible(false);
            }
            PanelAction::Toggle => {
                if !self.panel_visible {
                    debug::set_level(PanelDebugLevel::Off);
                }
                self.set_visible(!self.panel_visible);
            }
        }

        if request.debug != PanelDebugLevel::Off {
            // Debug level overrides apply immediately when requested via control plane.
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
            // Activate watches so widgets only poll while the panel is open.
            if let Some(volume) = self.volume.as_ref() {
                volume.set_watch_active(true);
            }
            if let Some(brightness) = self.brightness.as_ref() {
                brightness.set_watch_active(true);
            }
            if let Some(toggles) = self.toggles.as_ref() {
                toggles.set_watch_active(true);
            }
            // Pull focus so keyboard navigation starts inside the panel.
            self.panel.root.grab_focus();
            if let Some(handle) = self.media_handle.as_ref() {
                // Media refresh is deferred until the panel is visible.
                handle.refresh();
            }
            // Apply any pending media state that accumulated while the panel was hidden.
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
            // Flush deferred list rebuilds once to avoid repeated background work.
            if self.list_needs_rebuild() {
                // Apply any deferred list rebuilds once the panel becomes visible.
                self.list.flush_rebuild();
            }
            // Refresh counts after applying deferred updates to keep UI consistent.
            // Refresh counts after pending updates land so header stays accurate.
            self.refresh_counts();
            self.refresh_widgets(true);
            self.start_refresh_timer();
            // Resolve work-area margins before showing the window to avoid a layout shift.
            // This prevents a first-frame resize when Hyprland publishes margins after open.
            // Only hit the compositor once per open when the cache is empty.
            // Keeps open latency stable while avoiding repeated IPC work.
            if self.config.panel.respect_work_area && self.work_area.is_none() {
                self.work_area = super::hyprland::reserved_work_area_sync(
                    self.config.panel.output.as_deref(),
                );
                super::panel::apply_panel_config(&self.panel, &self.config, self.work_area);
            }
            // Only show the window after geometry is correct to avoid visible jitter.
            self.panel.window.set_visible(true);
            let width = self.panel.window.allocated_width();
            let height = self.panel.window.allocated_height();
            let message = format!("panel allocated size: {width}x{height}");
            self.log_debug(PanelDebugLevel::Verbose, move || message);
        } else {
            // Hide first so any teardown work does not trigger visible reflow.
            self.panel.window.set_visible(false);
            // Disable watch-based polling when hidden to reduce background load.
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

    pub(super) fn close_if_click_outside(&self) {
        if !self.panel_visible {
            return;
        }
        if !self.is_click_outside_panel() {
            self.log_debug(PanelDebugLevel::Verbose, || {
                "click outside ignored (pointer inside panel)".to_string()
            });
            return;
        }
        // Close requests go through the daemon to keep control state consistent.
        self.log_debug(PanelDebugLevel::Info, || {
            "click outside detected; requesting close".to_string()
        });
        // Non-blocking send avoids freezing the click handler.
        try_send_command(&self.command_tx, UiCommand::ClosePanel);
    }

    pub(super) fn log_debug(&self, level: PanelDebugLevel, message: impl FnOnce() -> String) {
        debug::log(level, message);
    }

    fn is_click_outside_panel(&self) -> bool {
        // Hyprland focus changes can be hover-driven; only close when a mouse button is down.
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
