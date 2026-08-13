//! Popup UI state construction and icon-source monitor setup

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use tokio::sync::mpsc::Sender;
use unixnotis_core::{Config, ControlState};
use unixnotis_ui::css::CssManager;
use unixnotis_ui::icons::DesktopIconIndex;

use crate::dbus::UiCommand;

use super::super::icons::{TextureCache, ThemeIconCache};
use super::super::window::build_popup_window;
use super::model::UiState;

impl UiState {
    pub fn new(
        app: &gtk::Application,
        config: Config,
        config_path: std::path::PathBuf,
        command_tx: Sender<UiCommand>,
        css: CssManager,
    ) -> Self {
        // Window construction returns the input-region state used by later reloads
        let (popup_window, popup_stack, popup_input_region) = build_popup_window(app, &config);

        let icon_sources_dirty = Rc::new(Cell::new(false));
        let app_info_monitor = gtk::gio::AppInfoMonitor::get();
        let dirty = Rc::clone(&icon_sources_dirty);
        app_info_monitor.connect_changed(move |_| dirty.set(true));
        let icon_theme = gtk::gdk::Display::default().map(|display| {
            let theme = gtk::IconTheme::for_display(&display);
            let dirty = Rc::clone(&icon_sources_dirty);
            theme.connect_changed(move |_| dirty.set(true));
            theme
        });

        Self {
            config,
            config_path,
            css,
            command_tx,
            popup_event_tx: None,
            popup_window,
            popup_stack,
            popup_input_region,
            popups: HashMap::new(),
            popup_order: VecDeque::new(),
            hidden_popups: std::collections::HashSet::new(),
            visible_popups: Vec::new(),
            // Startup remains permissive until the daemon seed arrives
            control_state: ControlState::default(),
            desktop_icons: DesktopIconIndex::new(),
            icon_sources_dirty,
            icon_source_generation: 0,
            _app_info_monitor: app_info_monitor,
            _icon_theme: icon_theme,
            icon_cache: HashMap::new(),
            icon_cache_order: VecDeque::new(),
            icon_texture_cache: Rc::new(RefCell::new(TextureCache::new_for_popups())),
            theme_icon_cache: ThemeIconCache::new_for_popups(),
        }
    }

    pub(crate) fn set_popup_event_sender(
        &mut self,
        sender: async_channel::Sender<crate::dbus::UiEvent>,
    ) {
        // The production event loop owns this sender; tests can leave it unset
        self.popup_event_tx = Some(sender);
    }
}
