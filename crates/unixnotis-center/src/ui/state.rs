//! Shared GTK state and construction inputs for the center UI

use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::mpsc;
use unixnotis_core::{Config, IconAssetResolver, Margins};
use unixnotis_ui::css::CssManager;

use crate::control::{UiCommand, UiEvent};

use super::{icons, media, notifications, panel, reload, widgets};

/// GTK state for the notification center panel
pub struct UiState {
    pub(super) config: Config,
    pub(super) config_path: std::path::PathBuf,
    pub(super) css: CssManager,
    // This owner must drop before the panel so its manually parented popover can detach
    pub(super) dnd_duration_menu: panel::header::dnd::DndDurationMenu,
    pub(super) panel: panel::widgets::PanelWidgets,
    pub(super) list: notifications::NotificationList,
    // Shared resolver keeps icon cache and inflight decode tracking centralized
    pub(super) icon_resolver: Rc<icons::IconResolver>,
    // Widget assets are resolved relative to the active config file root
    pub(super) widget_icon_resolver: IconAssetResolver,
    pub(super) dnd_guard: Rc<Cell<bool>>,
    // One countdown owns its deadline so completed GLib sources are never removed twice
    pub(super) dnd_expiration_source: Option<panel::header::dnd::DndCountdown>,
    pub(super) search_toggle_guard: Rc<Cell<bool>>,
    pub(super) panel_visible: bool,
    // A hidden panel defers list painting, so the next open must reveal the newest complete row
    pub(super) notifications_changed_while_hidden: bool,
    pub(super) panel_visible_flag: Arc<AtomicBool>,
    pub(super) work_area: Option<Margins>,
    // Tracks the last rendered counts to avoid redundant label updates
    pub(super) last_count: Option<notifications::NotificationCounts>,
    pub(super) media: Option<media::MediaWidget>,
    pub(super) media_handle: Option<crate::media::MediaHandle>,
    // Holds the most recent media snapshot while the panel is hidden
    // Defers GTK updates until visible to keep idle CPU near zero
    pub(super) pending_media: Option<Vec<crate::media::MediaInfo>>,
    // Tracks a pending media clear request while hidden
    // Ensures stale artwork does not linger across open/close cycles
    pub(super) pending_media_cleared: bool,
    pub(super) volume: Option<widgets::volume::VolumeWidget>,
    pub(super) brightness: Option<widgets::brightness::BrightnessWidget>,
    pub(super) toggles: Option<widgets::toggles::ToggleGrid>,
    pub(super) stats: Option<widgets::stats::StatGrid>,
    pub(super) cards: Option<widgets::cards::CardGrid>,
    pub(super) command_tx: mpsc::Sender<UiCommand>,
    pub(super) event_tx: async_channel::Sender<UiEvent>,
    pub(super) widgets_collapsed: bool,
    pub(super) refresh_source: Option<gtk::glib::SourceId>,
    pub(super) last_slow_refresh: Option<Instant>,
    // Separate config and CSS state preserves severity priority across watcher races
    pub(super) reload_notices: reload::ReloadNoticeState,
    // Keeps the shared async runtime alive for D-Bus and media tasks
    pub(super) _runtime: Arc<tokio::runtime::Runtime>,
}

/// Constructor inputs grouped to keep activation wiring readable
pub struct UiStateInit {
    pub app: gtk::Application,
    pub config: Config,
    pub config_path: std::path::PathBuf,
    pub command_tx: mpsc::Sender<UiCommand>,
    pub css: CssManager,
    pub event_tx: async_channel::Sender<UiEvent>,
    pub media_handle: Option<crate::media::MediaHandle>,
    pub runtime: Arc<tokio::runtime::Runtime>,
}

#[cfg(test)]
#[path = "tests/state.rs"]
mod tests;
