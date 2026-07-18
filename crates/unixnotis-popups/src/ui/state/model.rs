//! Popup UI state owned by the GTK main thread

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::time::Instant;

use tokio::sync::mpsc::Sender;
use unixnotis_core::{Config, ControlState};
use unixnotis_ui::css::CssManager;
use unixnotis_ui::icons::DesktopIconIndex;

use crate::dbus::UiCommand;

use super::super::entry::PopupEntry;
use super::super::icons::TextureCache;
use super::super::window::PopupInputRegionState;

/// Popup-only GTK state for notification toasts
pub struct UiState {
    pub(in crate::ui) config: Config,
    pub(in crate::ui) config_path: std::path::PathBuf,
    pub(in crate::ui) css: CssManager,
    pub(in crate::ui) command_tx: Sender<UiCommand>,
    pub(in crate::ui) popup_window: gtk::ApplicationWindow,
    pub(in crate::ui) popup_stack: gtk::Box,
    // Shared popup input shaping state for config and runtime updates
    pub(in crate::ui) popup_input_region: PopupInputRegionState,
    pub(in crate::ui) popups: HashMap<u32, PopupEntry>,
    pub(in crate::ui) popup_order: VecDeque<u32>,
    // Only visible ids need repeated GTK updates during backlog churn
    pub(in crate::ui) visible_popups: Vec<u32>,
    // Latest daemon gate state used to keep visible popups in policy
    pub(in crate::ui) control_state: ControlState,
    // Desktop icon index caches resolved icon themes for known applications
    pub(in crate::ui) desktop_icons: DesktopIconIndex,
    // Monitors mark lookup state dirty without rebuilding inside callbacks
    pub(in crate::ui) icon_sources_dirty: Rc<Cell<bool>>,
    pub(in crate::ui) _app_info_monitor: gtk::gio::AppInfoMonitor,
    pub(in crate::ui) _icon_theme: Option<gtk::IconTheme>,
    // Cache resolved icon names per app to reduce repeated theme lookups
    pub(in crate::ui) icon_cache: HashMap<String, IconCacheEntry>,
    // FIFO order used to cap icon cache growth
    pub(in crate::ui) icon_cache_order: VecDeque<String>,
    // Small LRU for decoded textures to avoid repeated PNG decode work
    pub(in crate::ui) icon_texture_cache: Rc<RefCell<TextureCache>>,
}

pub(in crate::ui) struct IconCacheEntry {
    pub(in crate::ui) resolved: Option<String>,
    pub(in crate::ui) cached_at: Instant,
}
