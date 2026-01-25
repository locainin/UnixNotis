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
use unixnotis_core::{Config, Urgency};

use crate::dbus::{UiCommand, UiEvent};
use unixnotis_ui::css::{self, CssManager};

use icons::DesktopIconIndex;
use ui_entry::PopupEntry;
use ui_icons::TextureCache;
use ui_window::{apply_popup_config, build_popup_window};

/// Popup-only GTK state for notification toasts.
pub struct UiState {
    config: Config,
    config_path: std::path::PathBuf,
    css: CssManager,
    command_tx: Sender<UiCommand>,
    popup_window: gtk::ApplicationWindow,
    popup_stack: gtk::Box,
    popups: HashMap<u32, PopupEntry>,
    popup_order: VecDeque<u32>,
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
        let (popup_window, popup_stack) = build_popup_window(app, &config);

        Self {
            config,
            config_path,
            css,
            command_tx,
            popup_window,
            popup_stack,
            popups: HashMap::new(),
            popup_order: VecDeque::new(),
            desktop_icons: DesktopIconIndex::new(),
            icon_cache: HashMap::new(),
            icon_cache_order: VecDeque::new(),
            icon_texture_cache: Rc::new(RefCell::new(TextureCache::new_for_popups())),
        }
    }

    pub fn handle_event(&mut self, event: UiEvent) {
        match event {
            UiEvent::Seed { state, active } => {
                if state.inhibited {
                    // Inhibits suppress popup rendering while preserving history elsewhere.
                    return;
                }
                if state.dnd_enabled {
                    for notification in active {
                        if notification.urgency == Urgency::Critical as u8 {
                            self.add_popup(notification);
                        }
                    }
                } else {
                    for notification in active {
                        self.add_popup(notification);
                    }
                }
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
                self.replace_popup(notification, show_popup);
            }
            UiEvent::NotificationClosed(id, _reason) => {
                debug!(id, "popup closed");
                self.remove_popup(id);
            }
            UiEvent::StateChanged(state) => {
                if state.inhibited {
                    debug!("clearing popups due to inhibition");
                    self.clear_popups();
                } else if state.dnd_enabled {
                    debug!("clearing popups due to dnd");
                    self.clear_popups();
                }
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
        let config = match Config::load_from_path(&self.config_path) {
            Ok(config) => config,
            Err(err) => {
                tracing::warn!(?err, "failed to reload config");
                return;
            }
        };
        let theme_base = self
            .config_path
            .parent()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                Config::default_config_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
            });
        let theme_paths = match config.resolve_theme_paths_from(&theme_base) {
            Ok(paths) => paths,
            Err(err) => {
                tracing::warn!(?err, "failed to resolve theme paths");
                return;
            }
        };

        self.config = config.clone();
        debug!("popup config reloaded");
        self.css.update_theme(theme_paths, config.theme.clone());
        self.css.reload(css::DEFAULT_CSS);
        apply_popup_config(&self.popup_window, &self.popup_stack, &config);
    }
}
