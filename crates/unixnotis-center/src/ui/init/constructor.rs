//! `UiState` construction flow

use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use unixnotis_core::{Config, PanelDebugLevel};

use super::super::{hyprland, icons, panel, widgets, UiState, UiStateInit};
use super::builders::{
    build_media_widget, build_notification_list, build_widget_sections, has_visible_widget_section,
    icon_resolver_for_widgets,
};
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
        let widget_icon_resolver = icon_resolver_for_widgets(&init.config_path);
        let media = build_media_widget(&panel, &init);
        let extra_widgets = build_widget_sections(&panel, &init, &widget_icon_resolver);
        list.set_empty_layout(has_visible_widget_section(&panel));

        panel::connect_dnd_toggle(&panel, dnd_guard.clone(), init.command_tx.clone());
        panel::connect_clear_button(&panel.clear_action_button, init.command_tx.clone());
        panel::connect_clear_button(&panel.clear_header_button, init.command_tx.clone());
        panel::connect_close_button(&panel, init.command_tx.clone());
        panel::connect_widget_collapse_toggle(&panel, init.event_tx.clone());
        panel::connect_filter_entry(&panel, init.event_tx.clone());
        panel::connect_search_toggle(&panel, search_toggle_guard.clone());
        panel::connect_auto_close(&panel, &init, panel_visible_flag.clone());
        panel::connect_keyboard_shortcuts(&panel, init.command_tx.clone());

        if init.config.panel.respect_work_area {
            // Work area is refreshed early to ensure the panel anchors correctly
            hyprland::refresh_reserved_work_area(
                init.config.panel.output.clone(),
                init.event_tx.clone(),
            );
        }

        // Long-lived state owns every channel, guard, and optional widget built above
        Self {
            config: init.config,
            config_path: init.config_path,
            css: init.css,
            panel,
            list,
            icon_resolver,
            widget_icon_resolver,
            dnd_guard,
            search_toggle_guard,
            panel_visible: false,
            panel_visible_flag,
            work_area: None,
            last_count: None,
            media,
            media_handle: init.media_handle,
            pending_media: None,
            // A separate cleared flag distinguishes no update from an explicit empty snapshot
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
            // Reload notices preserve independent config and CSS failure identities
            reload_notices: super::super::reload::ReloadNoticeState::default(),
            _runtime: init.runtime,
        }
    }
}
