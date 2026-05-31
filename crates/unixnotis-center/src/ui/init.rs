//! UI construction and initial wiring for `UiState`.
//!
//! Keeps the constructor focused on wiring GTK widgets, handlers, and runtime
//! state so other modules can focus on specialized behavior.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_channel::TrySendError;
use gtk::gdk;
use gtk::prelude::*;
use tracing::debug;
use unixnotis_core::{Config, PanelDebugLevel};

use crate::dbus::{UiCommand, UiEvent};
use crate::debug;

use super::input_guard::{ClickCooldown, LatestBoolEventGate};
use super::widget_builders::{build_extra_widgets, build_quick_controls};
use super::{
    hyprland, icons, list, media_widget, panel, try_send_command, widgets, UiState, UiStateInit,
};

const CONTROL_CLICK_GUARD_MS: u64 = 180;
const WIDGETS_TOGGLE_COALESCE_MS: u64 = 16;

fn connect_clear_button(button: &gtk::Button, command_tx: tokio::sync::mpsc::Sender<UiCommand>) {
    let clear_gate = ClickCooldown::new(Duration::from_millis(CONTROL_CLICK_GUARD_MS));
    button.connect_clicked(move |_| {
        if !clear_gate.try_start() {
            return;
        }
        debug!("clear all clicked");
        // Non-blocking send avoids UI stalls on D-Bus backpressure
        try_send_command(&command_tx, UiCommand::ClearAll);
    });
}

fn build_notification_list(
    panel: &panel::PanelWidgets,
    init: &UiStateInit,
    icon_resolver: Rc<icons::IconResolver>,
) -> list::NotificationList {
    let list_config = list::NotificationListConfig {
        max_active: init.config.history.max_active,
        max_entries: init.config.history.max_entries,
        transient_to_history: init.config.history.transient_to_history,
        show_notification_metadata: init.config.panel.notification_metadata_visible,
        show_notification_thumbnails: init.config.panel.notification_thumbnails_visible,
        empty_text: init.config.panel.empty_text.clone(),
        empty_offset_top: init.config.panel.empty_offset_top,
    };

    list::NotificationList::new(
        panel.scroller.clone(),
        init.command_tx.clone(),
        init.event_tx.clone(),
        icon_resolver,
        list_config,
    )
}

fn build_media_widget(
    panel: &panel::PanelWidgets,
    init: &UiStateInit,
) -> Option<media_widget::MediaWidget> {
    let panel_width = panel::live_panel_width(&panel.root);
    let media = init.media_handle.as_ref().map(|handle| {
        media_widget::MediaWidget::new(
            &panel.media_container,
            handle.clone(),
            panel_width,
            &init.config.media,
        )
    });
    if media.is_none() {
        panel.media_container.set_visible(false);
    }
    media
}

fn connect_dnd_toggle(
    panel: &panel::PanelWidgets,
    dnd_guard: Rc<Cell<bool>>,
    command_tx: tokio::sync::mpsc::Sender<UiCommand>,
) {
    panel.dnd_toggle.connect_toggled(move |button| {
        if dnd_guard.get() {
            // Daemon-driven state sync should not echo another DND command
            return;
        }
        debug!(enabled = button.is_active(), "dnd toggled");
        try_send_command(&command_tx, UiCommand::SetDnd(button.is_active()));
    });
}

fn connect_close_button(
    panel: &panel::PanelWidgets,
    command_tx: tokio::sync::mpsc::Sender<UiCommand>,
) {
    let close_gate = ClickCooldown::new(Duration::from_millis(CONTROL_CLICK_GUARD_MS));
    panel.close_button.connect_clicked(move |_| {
        if !close_gate.try_start() {
            return;
        }
        debug!("close panel clicked");
        try_send_command(&command_tx, UiCommand::ClosePanel);
    });
}

fn connect_widget_collapse_toggle(
    panel: &panel::PanelWidgets,
    event_tx: async_channel::Sender<UiEvent>,
) {
    let collapse_gate = LatestBoolEventGate::new(Duration::from_millis(WIDGETS_TOGGLE_COALESCE_MS));
    let collapse_click_gate =
        ClickCooldown::new(Duration::from_millis(panel::WIDGET_REVEAL_TRANSITION_MS));
    let accepted_collapsed = Rc::new(Cell::new(false));
    let collapse_restore = Rc::new(Cell::new(false));

    panel.focus_toggle.connect_toggled(move |button| {
        if collapse_restore.replace(false) {
            return;
        }

        let collapsed = button.is_active();
        if !collapse_click_gate.try_start() {
            let accepted = accepted_collapsed.get();
            if collapsed != accepted {
                // Roll back only the rejected edge so the UI mirrors the running transition
                collapse_restore.set(true);
                button.set_active(accepted);
            }
            return;
        }

        accepted_collapsed.set(collapsed);
        button.set_sensitive(false);
        let button_enable = button.clone();
        gtk::glib::timeout_add_local_once(
            Duration::from_millis(panel::WIDGET_REVEAL_TRANSITION_MS),
            move || {
                button_enable.set_sensitive(true);
            },
        );
        collapse_gate.request_widgets_collapsed(&event_tx, collapsed);
    });
}

fn connect_filter_entry(panel: &panel::PanelWidgets, event_tx: async_channel::Sender<UiEvent>) {
    panel.search_entry.connect_search_changed(move |entry| {
        let event = UiEvent::FilterChanged(entry.text().to_string());
        match event_tx.try_send(event) {
            Ok(()) => {}
            Err(TrySendError::Full(event)) => {
                // Search changes are small and should retry instead of disappearing under bursts
                let event_tx = event_tx.clone();
                gtk::glib::MainContext::default().spawn_local(async move {
                    let _ = event_tx.send(event).await;
                });
            }
            Err(TrySendError::Closed(_)) => {}
        }
    });
}

fn connect_search_toggle(panel: &panel::PanelWidgets, search_toggle_guard: Rc<Cell<bool>>) {
    let search_revealer = panel.search_revealer.clone();
    let search_entry = panel.search_entry.clone();
    let search_click_gate =
        ClickCooldown::new(Duration::from_millis(panel::SEARCH_REVEAL_TRANSITION_MS));
    let accepted_search_reveal = Rc::new(Cell::new(false));
    let search_restore = Rc::new(Cell::new(false));

    panel.search_toggle.connect_toggled(move |button| {
        if search_toggle_guard.get() || search_restore.replace(false) {
            return;
        }

        let reveal = button.is_active();
        if !search_click_gate.try_start() {
            let accepted = accepted_search_reveal.get();
            if reveal != accepted {
                // Keep the visual toggle synced with the accepted revealer state
                search_restore.set(true);
                button.set_active(accepted);
            }
            return;
        }

        accepted_search_reveal.set(reveal);
        button.set_sensitive(false);
        let button_enable = button.clone();
        gtk::glib::timeout_add_local_once(
            Duration::from_millis(panel::SEARCH_REVEAL_TRANSITION_MS),
            move || {
                button_enable.set_sensitive(true);
            },
        );
        search_revealer.set_reveal_child(reveal);
        if reveal {
            search_entry.grab_focus();
            search_entry.select_region(0, -1);
        } else if !search_entry.text().is_empty() {
            search_entry.set_text("");
        }
    });
}

fn connect_blur_close(
    command_tx: tokio::sync::mpsc::Sender<UiCommand>,
    visible_flag: Arc<AtomicBool>,
    window: &gtk::ApplicationWindow,
) {
    window.connect_is_active_notify(move |window| {
        if visible_flag.load(Ordering::SeqCst) && !window.is_active() {
            try_send_command(&command_tx, UiCommand::ClosePanel);
        }
    });
}

fn connect_auto_close(
    panel: &panel::PanelWidgets,
    init: &UiStateInit,
    visible_flag: Arc<AtomicBool>,
) {
    if init.config.panel.close_on_click_outside {
        let started =
            hyprland::start_active_window_watcher(init.event_tx.clone(), visible_flag.clone());
        if !started && init.config.panel.close_on_blur {
            connect_blur_close(init.command_tx.clone(), visible_flag, &panel.window);
        }
    } else if init.config.panel.close_on_blur {
        connect_blur_close(init.command_tx.clone(), visible_flag, &panel.window);
    }
}

fn connect_keyboard_shortcuts(
    panel: &panel::PanelWidgets,
    command_tx: tokio::sync::mpsc::Sender<UiCommand>,
) {
    let focus_toggle = panel.focus_toggle.clone();
    let search_toggle = panel.search_toggle.clone();
    let search_revealer = panel.search_revealer.clone();
    let search_entry = panel.search_entry.clone();
    let scroller = panel.scroller.clone();
    let key_controller = gtk::EventControllerKey::new();

    key_controller.connect_key_pressed(move |_, key, _, state| {
        if key == gdk::Key::Escape {
            if search_toggle.is_active() {
                // First escape closes search to avoid accidental panel dismissal
                search_toggle.set_active(false);
                return gtk::glib::Propagation::Stop;
            }
            try_send_command(&command_tx, UiCommand::ClosePanel);
            return gtk::glib::Propagation::Stop;
        }
        if key == gdk::Key::slash
            || (key == gdk::Key::f && state.contains(gdk::ModifierType::CONTROL_MASK))
        {
            if !search_revealer.reveals_child() {
                search_toggle.set_active(true);
            }
            search_entry.grab_focus();
            search_entry.select_region(0, -1);
            return gtk::glib::Propagation::Stop;
        }
        if key == gdk::Key::l && state.contains(gdk::ModifierType::CONTROL_MASK) {
            if !search_revealer.reveals_child() {
                search_toggle.set_active(true);
            }
            search_entry.set_text("");
            search_entry.grab_focus();
            return gtk::glib::Propagation::Stop;
        }
        if key == gdk::Key::w && state.contains(gdk::ModifierType::CONTROL_MASK) {
            focus_toggle.set_active(!focus_toggle.is_active());
            return gtk::glib::Propagation::Stop;
        }
        if !search_entry.has_focus() && (key == gdk::Key::j || key == gdk::Key::k) {
            let adjustment = scroller.vadjustment();
            let delta = if key == gdk::Key::j { 72.0 } else { -72.0 };
            let upper = (adjustment.upper() - adjustment.page_size()).max(adjustment.lower());
            let next = (adjustment.value() + delta).clamp(adjustment.lower(), upper);
            adjustment.set_value(next);
            return gtk::glib::Propagation::Stop;
        }
        gtk::glib::Propagation::Proceed
    });
    panel.root.add_controller(key_controller);
}

impl UiState {
    pub fn new(init: UiStateInit) -> Self {
        if let Ok(config_dir) = Config::config_dir_for_path(&init.config_path) {
            widgets::configure_command_config_dir(config_dir);
        }

        // Build the panel widget tree first so child widgets can be attached safely
        let panel = panel::build_panel_widgets(&init.app, &init.config);
        let icon_resolver = Rc::new(icons::IconResolver::new());
        debug::set_level(PanelDebugLevel::Off);
        let list = build_notification_list(&panel, &init, icon_resolver.clone());

        let dnd_guard = Rc::new(Cell::new(false));
        let search_toggle_guard = Rc::new(Cell::new(false));
        let panel_visible_flag = Arc::new(AtomicBool::new(false));
        let media = build_media_widget(&panel, &init);
        let (volume, brightness) = build_quick_controls(&panel, &init.config);
        let (toggles, stats, cards) = build_extra_widgets(&panel, &init.config);
        let has_widgets = panel.quick_controls.get_visible()
            || panel.media_container.get_visible()
            || panel.toggle_container.get_visible()
            || panel.stat_container.get_visible()
            || panel.card_container.get_visible();
        list.set_empty_layout(has_widgets);

        connect_dnd_toggle(&panel, dnd_guard.clone(), init.command_tx.clone());
        connect_clear_button(&panel.clear_action_button, init.command_tx.clone());
        connect_clear_button(&panel.clear_header_button, init.command_tx.clone());
        connect_close_button(&panel, init.command_tx.clone());
        connect_widget_collapse_toggle(&panel, init.event_tx.clone());
        connect_filter_entry(&panel, init.event_tx.clone());
        connect_search_toggle(&panel, search_toggle_guard.clone());
        connect_auto_close(&panel, &init, panel_visible_flag.clone());
        connect_keyboard_shortcuts(&panel, init.command_tx.clone());

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
            search_toggle_guard,
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
            widgets_collapsed: false,
            refresh_source: None,
            last_fast_refresh: None,
            last_slow_refresh: None,
            _runtime: init.runtime,
        }
    }
}
