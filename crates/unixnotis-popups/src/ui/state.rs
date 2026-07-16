//! Popup UI state and top-level event handling

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::time::Instant;

use tokio::sync::mpsc::Sender;
use tracing::debug;
use unixnotis_core::{Config, ControlState};

use crate::dbus::{UiCommand, UiEvent};
use unixnotis_ui::css::{self, CssManager};

use super::entry::PopupEntry;
use super::icons::TextureCache;
use super::window::{apply_popup_config, build_popup_window, PopupInputRegionState};
use unixnotis_ui::icons::DesktopIconIndex;

/// Popup-only GTK state for notification toasts
pub struct UiState {
    pub(super) config: Config,
    pub(super) config_path: std::path::PathBuf,
    pub(super) css: CssManager,
    pub(super) command_tx: Sender<UiCommand>,
    pub(super) popup_window: gtk::ApplicationWindow,
    pub(super) popup_stack: gtk::Box,
    // Shared popup input shaping state for config + runtime updates
    pub(super) popup_input_region: PopupInputRegionState,
    pub(super) popups: HashMap<u32, PopupEntry>,
    pub(super) popup_order: VecDeque<u32>,
    // Only visible ids need repeated GTK updates during backlog churn
    pub(super) visible_popups: Vec<u32>,
    // Latest daemon gate state used to keep visible popups in policy
    pub(super) control_state: ControlState,
    // Desktop icon index caches resolved icon themes for known applications
    pub(super) desktop_icons: DesktopIconIndex,
    // Desktop and icon-theme monitors mark lookup state dirty without rebuilding in callbacks
    pub(super) icon_sources_dirty: Rc<Cell<bool>>,
    _app_info_monitor: gtk::gio::AppInfoMonitor,
    _icon_theme: Option<gtk::IconTheme>,
    // Cache resolved icon names per app to reduce repeated theme lookups
    pub(super) icon_cache: HashMap<String, IconCacheEntry>,
    // FIFO order used to cap icon cache growth
    pub(super) icon_cache_order: VecDeque<String>,
    // Small LRU for decoded textures to avoid repeated PNG decode work
    pub(super) icon_texture_cache: Rc<RefCell<TextureCache>>,
}

impl UiState {
    pub fn new(
        app: &gtk::Application,
        config: Config,
        config_path: std::path::PathBuf,
        command_tx: Sender<UiCommand>,
        css: CssManager,
    ) -> Self {
        // Build window and region state together so callbacks share one source
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
            popup_window,
            popup_stack,
            popup_input_region,
            popups: HashMap::new(),
            popup_order: VecDeque::new(),
            visible_popups: Vec::new(),
            // Start permissive until the first seed arrives from the daemon
            control_state: ControlState::default(),
            desktop_icons: DesktopIconIndex::new(),
            icon_sources_dirty,
            _app_info_monitor: app_info_monitor,
            _icon_theme: icon_theme,
            icon_cache: HashMap::new(),
            icon_cache_order: VecDeque::new(),
            icon_texture_cache: Rc::new(RefCell::new(TextureCache::new_for_popups())),
        }
    }

    pub fn handle_event(&mut self, event: UiEvent) {
        match event {
            UiEvent::Seed { state, active } => {
                // Seed is the daemon truth, so local popup state must reconcile to it
                // Control state is stored first so seed filtering uses the newest gate
                self.control_state = state;
                self.reconcile_seed(active);
            }
            UiEvent::NotificationAdded(notification, show_popup) => {
                if show_popup {
                    debug!(
                        id = notification.id,
                        app = %notification.app_name,
                        "popup added"
                    );
                    self.add_popup(notification);
                }
            }
            UiEvent::NotificationUpdated(notification, show_popup) => {
                debug!(
                    id = notification.id,
                    app = %notification.app_name,
                    "popup updated"
                );
                self.update_popup(notification, show_popup);
            }
            UiEvent::NotificationClosed(id, _reason) => {
                debug!(id, "popup closed");
                self.remove_popup(id);
            }
            UiEvent::PopupGateChanged(gate) => {
                // Popup policy only depends on DND and inhibit state
                self.control_state.dnd_enabled = gate.dnd_enabled;
                self.control_state.inhibited = gate.inhibited;
                self.retain_allowed_popups();
            }
            UiEvent::CssReload => {
                debug!("popup css reload requested");
                let report = self.css.reload(css::DEFAULT_CSS);
                super::css_reload::log_reload_failures(&report, "watcher reload");
                self.invalidate_icon_sources();
            }
            UiEvent::ConfigReload => {
                debug!("popup config reload requested");
                self.reload_config();
            }
        }
    }

    fn reload_config(&mut self) {
        // Config reload must fail soft so popup runtime stays alive on parse errors
        let config = match Config::load_from_path(&self.config_path) {
            Ok(config) => config,
            Err(err) => {
                tracing::warn!(?err, "failed to reload config");
                return;
            }
        };
        // Theme resolution uses config directory as the base for relative paths
        let theme_base = match Config::config_dir_for_path(&self.config_path) {
            Ok(path) => path,
            Err(err) => {
                tracing::warn!(?err, "failed to resolve config dir");
                return;
            }
        };
        // Theme path errors are reported without interrupting existing runtime state
        let theme_paths = match config.resolve_theme_paths_from(&theme_base) {
            Ok(paths) => paths,
            Err(err) => {
                tracing::warn!(?err, "failed to resolve theme paths");
                return;
            }
        };

        // Swap config first so follow-up apply calls read coherent values
        self.config = config.clone();
        debug!("popup config reloaded");
        // CSS updates are applied before window geometry so visual updates are atomic
        self.css.update_theme(theme_paths, config.theme.clone());
        let report = self.css.reload(css::DEFAULT_CSS);
        super::css_reload::log_reload_failures(&report, "config reload");
        self.invalidate_icon_sources();
        apply_popup_config(
            &self.popup_window,
            &self.popup_stack,
            &config,
            &self.popup_input_region,
        );
        // Config reload must also refresh visible popup widgets and limits
        self.refresh_after_config_reload();
    }
}

pub(super) struct IconCacheEntry {
    pub(super) resolved: Option<String>,
    pub(super) cached_at: Instant,
}

#[cfg(test)]
#[path = "tests/state.rs"]
mod tests;
