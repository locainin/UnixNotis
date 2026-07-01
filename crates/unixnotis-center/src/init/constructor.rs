//! `UiState` construction flow

use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use unixnotis_core::{Config, PanelDebugLevel};

use super::super::{hyprland, icons, panel, widgets, UiState, UiStateInit};
use super::actions::{connect_clear_button, connect_close_button, connect_dnd_toggle};
use super::autoclose::connect_auto_close;
use super::builders::{
    build_media_widget, build_notification_list, build_widget_sections, has_visible_widget_section,
};
use super::keyboard::connect_keyboard_shortcuts;
use super::search::{connect_filter_entry, connect_search_toggle, connect_widget_collapse_toggle};
use crate::debug;

impl UiState {
    pub fn new(init: UiStateInit) -> Self {
        if let Ok(config_dir) = Config::config_dir_for_path(&init.config_path) {
            // Widget command helpers resolve relative scripts against the active config root
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
        let extra_widgets = build_widget_sections(&panel, &init);
        list.set_empty_layout(has_visible_widget_section(&panel));

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
            // Work area is refreshed early to ensure the panel anchors correctly
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
            volume: extra_widgets.volume,
            brightness: extra_widgets.brightness,
            toggles: extra_widgets.toggles,
            stats: extra_widgets.stats,
            cards: extra_widgets.cards,
            command_tx: init.command_tx,
            event_tx: init.event_tx,
            widgets_collapsed: false,
            refresh_source: None,
            last_slow_refresh: None,
            _runtime: init.runtime,
        }
    }
}
