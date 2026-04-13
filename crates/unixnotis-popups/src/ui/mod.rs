//! Popup UI state, layout, and event handling.
//!
//! Keeps popup layout, icon decode, and list management in focused modules.

// Load the shared icon helpers without pulling them into the crate root module list.
#[path = "../icons/mod.rs"]
mod icons;
mod ui_entry;
mod ui_icons;
mod ui_popups;
mod ui_window;

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use tokio::sync::mpsc::Sender;
use tracing::debug;
use unixnotis_core::{Config, ControlState};

use crate::dbus::{UiCommand, UiEvent};
use unixnotis_ui::css::{self, CssManager};

use icons::DesktopIconIndex;
use ui_entry::PopupEntry;
use ui_icons::TextureCache;
use ui_window::{apply_popup_config, build_popup_window, PopupInputRegionState};

/// Popup-only GTK state for notification toasts.
pub struct UiState {
    config: Config,
    config_path: std::path::PathBuf,
    css: CssManager,
    command_tx: Sender<UiCommand>,
    popup_window: gtk::ApplicationWindow,
    popup_stack: gtk::Box,
    // Shared popup input shaping state for config + runtime updates
    popup_input_region: PopupInputRegionState,
    popups: HashMap<u32, PopupEntry>,
    popup_order: VecDeque<u32>,
    // Only visible ids need repeated GTK updates during backlog churn
    visible_popups: Vec<u32>,
    // Latest daemon gate state used to keep visible popups in policy
    control_state: ControlState,
    // Desktop icon index caches resolved icon themes for known applications.
    desktop_icons: DesktopIconIndex,
    // Cache resolved icon names per app to reduce repeated theme lookups.
    icon_cache: HashMap<String, Option<String>>,
    // FIFO order used to cap icon cache growth.
    icon_cache_order: VecDeque<String>,
    // Small LRU for decoded textures to avoid repeated PNG decode work.
    icon_texture_cache: Rc<RefCell<TextureCache>>,
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
                self.css.reload(css::DEFAULT_CSS);
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
        self.css.reload(css::DEFAULT_CSS);
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
