//! Center UI state, widget wiring, and event handling.
//!
//! Implementation is split across focused modules to keep this root file concise
//! while preserving a single home for shared state definitions.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::mpsc;
use unixnotis_core::{Config, Margins};

use crate::dbus::{UiCommand, UiEvent};
use unixnotis_ui::css::CssManager;

mod config;
mod events;
mod hyprland;
mod icons;
mod init;
mod list;
mod marquee;
mod media_widget;
mod panel;
mod refresh;
mod visibility;
mod widget_builders;
mod widgets;

/// GTK state for the notification center panel.
pub struct UiState {
    config: Config,
    config_path: std::path::PathBuf,
    css: CssManager,
    panel: panel::PanelWidgets,
    list: list::NotificationList,
    dnd_guard: Rc<Cell<bool>>,
    panel_visible: bool,
    panel_visible_flag: Arc<AtomicBool>,
    work_area: Option<Margins>,
    media: Option<media_widget::MediaWidget>,
    media_handle: Option<crate::media::MediaHandle>,
    volume: Option<widgets::volume::VolumeWidget>,
    brightness: Option<widgets::brightness::BrightnessWidget>,
    toggles: Option<widgets::toggles::ToggleGrid>,
    stats: Option<widgets::stats::StatGrid>,
    cards: Option<widgets::cards::CardGrid>,
    command_tx: mpsc::Sender<UiCommand>,
    event_tx: async_channel::Sender<UiEvent>,
    refresh_source: Option<gtk::glib::SourceId>,
    last_fast_refresh: Option<Instant>,
    last_slow_refresh: Option<Instant>,
    // Keeps the shared async runtime alive for D-Bus and media tasks.
    _runtime: Arc<tokio::runtime::Runtime>,
}

// Bundles constructor inputs to keep initialization readable and stable.
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

pub(super) fn try_send_command(command_tx: &mpsc::Sender<UiCommand>, command: UiCommand) {
    // Non-blocking send keeps GTK handlers responsive under D-Bus stalls.
    if let Err(err) = command_tx.try_send(command) {
        // Dropping commands on backpressure is safer than blocking the UI thread.
        tracing::warn!(?err, "failed to enqueue ui command");
    }
}
