//! UI construction and initial wiring for `UiState`.
//!
//! Keeps the constructor focused on wiring GTK widgets, handlers, and runtime
//! state so other modules can focus on specialized behavior.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use gtk::gdk;
use gtk::prelude::*;
use tracing::debug;
use unixnotis_core::PanelDebugLevel;

use crate::dbus::UiCommand;
use crate::debug;

use super::widget_builders::{build_extra_widgets, build_quick_controls};
use super::{hyprland, icons, list, media_widget, panel, try_send_command, UiState, UiStateInit};

impl UiState {
    pub fn new(init: UiStateInit) -> Self {
        // Build the panel widget tree first so child widgets can be attached safely.
        let panel = panel::build_panel_widgets(&init.app, &init.config);
        let icon_resolver = Rc::new(icons::IconResolver::new());
        debug::set_level(PanelDebugLevel::Off);
        // List rendering is initialized with the current config limits and shared icon resolver.
        let list_config = list::NotificationListConfig {
            max_active: init.config.history.max_active,
            max_entries: init.config.history.max_entries,
            empty_text: init.config.panel.empty_text.clone(),
            empty_offset_top: init.config.panel.empty_offset_top,
        };
        let list = list::NotificationList::new(
            panel.scroller.clone(),
            init.command_tx.clone(),
            init.event_tx.clone(),
            icon_resolver.clone(),
            list_config,
        );

        // DND updates are triggered from both UI and daemon; guard prevents feedback loops.
        let dnd_guard = Rc::new(Cell::new(false));
        let panel_visible_flag = Arc::new(AtomicBool::new(false));
        // Media widget is optional; keep the container hidden when no media handle exists.
        let media = init.media_handle.as_ref().map(|handle| {
            media_widget::MediaWidget::new(
                &panel.media_container,
                handle.clone(),
                init.config.panel.width,
                init.config.media.title_char_limit,
            )
        });
        if media.is_none() {
            panel.media_container.set_visible(false);
        }
        let (volume, brightness) = build_quick_controls(&panel, &init.config);
        let (toggles, stats, cards) = build_extra_widgets(&panel, &init.config);
        let has_widgets = panel.quick_controls.is_visible()
            || panel.media_container.is_visible()
            || panel.toggle_container.is_visible()
            || panel.stat_container.is_visible()
            || panel.card_container.is_visible();
        list.set_empty_layout(has_widgets);

        let dnd_guard_clone = dnd_guard.clone();
        let dnd_tx = init.command_tx.clone();
        panel.dnd_toggle.connect_toggled(move |button| {
            if dnd_guard_clone.get() {
                // Ignore UI-initiated toggles while applying daemon-driven state.
                return;
            }
            debug!(enabled = button.is_active(), "dnd toggled");
            // Non-blocking send keeps GTK handlers responsive.
            try_send_command(&dnd_tx, UiCommand::SetDnd(button.is_active()));
        });

        let clear_tx = init.command_tx.clone();
        panel.clear_button.connect_clicked(move |_| {
            debug!("clear all clicked");
            // Non-blocking send avoids UI stalls on D-Bus backpressure.
            try_send_command(&clear_tx, UiCommand::ClearAll);
        });

        let close_tx = init.command_tx.clone();
        panel.close_button.connect_clicked(move |_| {
            debug!("close panel clicked");
            // Best-effort enqueue keeps close behavior immediate.
            try_send_command(&close_tx, UiCommand::ClosePanel);
        });

        let connect_blur_close =
            |close_tx: tokio::sync::mpsc::Sender<UiCommand>,
             visible_flag: Arc<AtomicBool>,
             window: &gtk::ApplicationWindow| {
                // Focus-based close is shared between click-away fallback and explicit blur mode.
                window.connect_is_active_notify(move |window| {
                    // Only close when the panel is visible and focus is lost.
                    if !visible_flag.load(Ordering::SeqCst) {
                        return;
                    }
                    if !window.is_active() {
                        try_send_command(&close_tx, UiCommand::ClosePanel);
                    }
                });
            };

        if init.config.panel.close_on_click_outside {
            // Hyprland watcher is preferred; fall back to focus-based close if unavailable.
            // Hyprland watcher emits active-window changes that are later filtered for clicks.
            let started = hyprland::start_active_window_watcher(
                init.event_tx.clone(),
                panel_visible_flag.clone(),
            );
            if !started && init.config.panel.close_on_blur {
                connect_blur_close(
                    init.command_tx.clone(),
                    panel_visible_flag.clone(),
                    &panel.window,
                );
            }
        } else if init.config.panel.close_on_blur {
            connect_blur_close(
                init.command_tx.clone(),
                panel_visible_flag.clone(),
                &panel.window,
            );
        }

        // Escape closes the panel regardless of the focused widget.
        let esc_tx = init.command_tx.clone();
        let key_controller = gtk::EventControllerKey::new();
        key_controller.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape {
                // Escape should close quickly without blocking the UI thread.
                try_send_command(&esc_tx, UiCommand::ClosePanel);
                return gtk::glib::Propagation::Stop;
            }
            gtk::glib::Propagation::Proceed
        });
        panel.root.add_controller(key_controller);

        if init.config.panel.respect_work_area {
            // Work area is refreshed early to ensure the panel anchors correctly.
            hyprland::refresh_reserved_work_area(
                init.config.panel.output.clone(),
                init.event_tx.clone(),
            );
        }

        Self {
            config: init.config,
            config_path: init.config_path,
            css: init.css,
            panel,
            list,
            icon_resolver,
            dnd_guard,
            panel_visible: false,
            panel_visible_flag,
            work_area: None,
            last_count: None,
            media,
            media_handle: init.media_handle,
            pending_media: None,
            pending_media_cleared: false,
            volume,
            brightness,
            toggles,
            stats,
            cards,
            command_tx: init.command_tx,
            event_tx: init.event_tx,
            refresh_source: None,
            last_fast_refresh: None,
            last_slow_refresh: None,
            _runtime: init.runtime,
        }
    }
}
