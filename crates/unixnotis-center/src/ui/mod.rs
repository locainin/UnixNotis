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
use tokio::sync::mpsc::error::TrySendError;
use unixnotis_core::{Config, Margins};

use crate::dbus::{UiCommand, UiEvent};
use unixnotis_ui::css::CssManager;

mod config;
mod config_media;
mod events;
mod hyprland;
mod icons;
mod init;
mod input_guard;
mod list;
mod marquee;
mod media_art;
mod media_widget;
mod panel;
mod perf_probe;
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
    // Shared resolver keeps icon cache and inflight decode tracking centralized.
    icon_resolver: Rc<icons::IconResolver>,
    dnd_guard: Rc<Cell<bool>>,
    search_toggle_guard: Rc<Cell<bool>>,
    panel_visible: bool,
    panel_visible_flag: Arc<AtomicBool>,
    work_area: Option<Margins>,
    // Tracks the last rendered count to avoid redundant label updates.
    last_count: Option<usize>,
    media: Option<media_widget::MediaWidget>,
    media_handle: Option<crate::media::MediaHandle>,
    // Holds the most recent media snapshot while the panel is hidden.
    // Defers GTK updates until visible to keep idle CPU near zero.
    pending_media: Option<Vec<crate::media::MediaInfo>>,
    // Tracks a pending media clear request while hidden.
    // Ensures stale artwork does not linger across open/close cycles.
    pending_media_cleared: bool,
    volume: Option<widgets::volume::VolumeWidget>,
    brightness: Option<widgets::brightness::BrightnessWidget>,
    toggles: Option<widgets::toggles::ToggleGrid>,
    stats: Option<widgets::stats::StatGrid>,
    cards: Option<widgets::cards::CardGrid>,
    command_tx: mpsc::Sender<UiCommand>,
    event_tx: async_channel::Sender<UiEvent>,
    widgets_collapsed: bool,
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

impl UiState {
    pub fn panel_is_visible(&self) -> bool {
        self.panel_visible
    }
}

pub(super) fn try_send_command(command_tx: &mpsc::Sender<UiCommand>, command: UiCommand) {
    // Non-blocking send keeps GTK handlers responsive under D-Bus stalls.
    match command_tx.try_send(command) {
        Ok(()) => {}
        Err(TrySendError::Full(command)) => {
            // Backpressure is retried asynchronously to avoid dropping user actions.
            let command_tx = command_tx.clone();
            glib::MainContext::default().spawn_local(async move {
                if let Err(err) = command_tx.send(command).await {
                    tracing::warn!(?err, "failed to enqueue ui command after backpressure");
                }
            });
        }
        Err(TrySendError::Closed(command)) => {
            tracing::warn!(?command, "ui command dropped because channel closed");
        }
    }
}
