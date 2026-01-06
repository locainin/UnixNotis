//! Center UI state, widget wiring, and event handling.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use gtk::gdk;
use gtk::prelude::*;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, info};
use unixnotis_core::{Config, Margins, PanelDebugLevel, PanelRequest};

use crate::dbus::{UiCommand, UiEvent};
use crate::debug;
use unixnotis_ui::css::{self, CssManager};

mod hyprland;
mod icons;
mod list;
mod list_item;
mod marquee;
mod media_widget;
mod panel;
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
    command_tx: UnboundedSender<UiCommand>,
    event_tx: async_channel::Sender<UiEvent>,
    refresh_source: Option<gtk::glib::SourceId>,
    last_fast_refresh: Option<Instant>,
    last_slow_refresh: Option<Instant>,
}

impl UiState {
    pub fn new(
        app: &gtk::Application,
        config: Config,
        config_path: std::path::PathBuf,
        command_tx: UnboundedSender<UiCommand>,
        css: CssManager,
        event_tx: async_channel::Sender<UiEvent>,
        media_handle: Option<crate::media::MediaHandle>,
    ) -> Self {
        let panel = panel::build_panel_widgets(app, &config);
        let icon_resolver = Rc::new(icons::IconResolver::new());
        debug::set_level(PanelDebugLevel::Off);
        let list = list::NotificationList::new(
            panel.scroller.clone(),
            command_tx.clone(),
            event_tx.clone(),
            icon_resolver,
        );

        let dnd_guard = Rc::new(Cell::new(false));
        let panel_visible_flag = Arc::new(AtomicBool::new(false));
        let media = media_handle.as_ref().map(|handle| {
            media_widget::MediaWidget::new(
                &panel.media_container,
                handle.clone(),
                config.panel.width,
                config.media.title_char_limit,
            )
        });
        if media.is_none() {
            panel.media_container.set_visible(false);
        }
        let (volume, brightness) = build_quick_controls(&panel, &config);
        let (toggles, stats, cards) = build_extra_widgets(&panel, &config);
        let dnd_guard_clone = dnd_guard.clone();
        let dnd_tx = command_tx.clone();
        panel.dnd_toggle.connect_toggled(move |button| {
            if dnd_guard_clone.get() {
                return;
            }
            debug!(enabled = button.is_active(), "dnd toggled");
            let _ = dnd_tx.send(UiCommand::SetDnd(button.is_active()));
        });

        let clear_tx = command_tx.clone();
        panel.clear_button.connect_clicked(move |_| {
            debug!("clear all clicked");
            let _ = clear_tx.send(UiCommand::ClearAll);
        });

        let close_tx = command_tx.clone();
        panel.close_button.connect_clicked(move |_| {
            debug!("close panel clicked");
            let _ = close_tx.send(UiCommand::ClosePanel);
        });

        if config.panel.close_on_click_outside {
            // Hyprland watcher emits active-window changes that are later filtered for clicks.
            let started =
                hyprland::start_active_window_watcher(event_tx.clone(), panel_visible_flag.clone());
            if !started && config.panel.close_on_blur {
                let close_tx = command_tx.clone();
                let visible_flag = panel_visible_flag.clone();
                panel.window.connect_is_active_notify(move |window| {
                    if !visible_flag.load(Ordering::SeqCst) {
                        return;
                    }
                    if !window.is_active() {
                        let _ = close_tx.send(UiCommand::ClosePanel);
                    }
                });
            }
        } else if config.panel.close_on_blur {
            let close_tx = command_tx.clone();
            let visible_flag = panel_visible_flag.clone();
            panel.window.connect_is_active_notify(move |window| {
                if !visible_flag.load(Ordering::SeqCst) {
                    return;
                }
                if !window.is_active() {
                    let _ = close_tx.send(UiCommand::ClosePanel);
                }
            });
        }

        let esc_tx = command_tx.clone();
        let key_controller = gtk::EventControllerKey::new();
        key_controller.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape {
                let _ = esc_tx.send(UiCommand::ClosePanel);
                return gtk::glib::Propagation::Stop;
            }
            gtk::glib::Propagation::Proceed
        });
        panel.root.add_controller(key_controller);

        if config.panel.respect_work_area {
            hyprland::refresh_reserved_work_area(config.panel.output.clone(), event_tx.clone());
        }

        Self {
            config,
            config_path,
            css,
            panel,
            list,
            dnd_guard,
            panel_visible: false,
            panel_visible_flag,
            work_area: None,
            media,
            media_handle,
            volume,
            brightness,
            toggles,
            stats,
            cards,
            command_tx,
            event_tx,
            refresh_source: None,
            last_fast_refresh: None,
            last_slow_refresh: None,
        }
    }

    pub fn handle_event(&mut self, event: UiEvent) {
        match event {
            UiEvent::Seed {
                state,
                active,
                history,
            } => {
                debug!(
                    active = active.len(),
                    history = history.len(),
                    "received initial state"
                );
                self.list.seed(active, history);
                self.update_state(state);
                self.refresh_counts();
            }
            UiEvent::NotificationAdded(notification, _show_popup) => {
                debug!(
                    id = notification.id,
                    app = %notification.app_name,
                    "notification added"
                );
                self.log_debug(
                    PanelDebugLevel::Verbose,
                    || format!("notification added: {} #{}", notification.app_name, notification.id),
                );
                self.list.add_or_update(notification, true);
                self.refresh_counts();
            }
            UiEvent::NotificationUpdated(notification, _show_popup) => {
                debug!(
                    id = notification.id,
                    app = %notification.app_name,
                    "notification updated"
                );
                self.log_debug(
                    PanelDebugLevel::Verbose,
                    || {
                        format!(
                            "notification updated: {} #{}",
                            notification.app_name, notification.id
                        )
                    },
                );
                self.list.add_or_update(notification, true);
                self.refresh_counts();
            }
            UiEvent::NotificationClosed(id, reason) => {
                debug!(id, ?reason, "notification closed");
                self.log_debug(
                    PanelDebugLevel::Verbose,
                    || format!("notification closed: #{id} ({reason:?})"),
                );
                self.list.mark_closed(id, reason);
                self.refresh_counts();
            }
            UiEvent::StateChanged(state) => {
                debug!(dnd = state.dnd_enabled, "state updated");
                self.log_debug(
                    PanelDebugLevel::Info,
                    || format!("state changed: dnd={}", state.dnd_enabled),
                );
                self.update_state(state);
                self.refresh_counts();
            }
            UiEvent::PanelRequested(request) => {
                debug!(?request, "panel request");
                self.log_debug(
                    PanelDebugLevel::Info,
                    || format!("panel request: {:?}", request),
                );
                self.apply_panel_request(request);
            }
            UiEvent::GroupToggled(key) => {
                debug!(app = %key, "group toggled");
                self.log_debug(
                    PanelDebugLevel::Verbose,
                    || format!("group toggled: {key}"),
                );
                self.list.toggle_group(&key);
                self.refresh_counts();
            }
            UiEvent::MediaUpdated(infos) => {
                debug!(players = infos.len(), "media updated");
                self.log_debug(
                    PanelDebugLevel::Verbose,
                    || format!("media updated: {} players", infos.len()),
                );
                if let Some(widget) = self.media.as_mut() {
                    widget.update(&infos);
                }
            }
            UiEvent::MediaCleared => {
                debug!("media cleared");
                self.log_debug(PanelDebugLevel::Info, || "media cleared".to_string());
                if let Some(widget) = self.media.as_mut() {
                    widget.clear();
                }
            }
            UiEvent::ClickOutside => {
                debug!("click outside detected");
                self.close_if_click_outside();
            }
            UiEvent::WorkAreaUpdated(reserved) => {
                debug!(?reserved, "work area updated");
                self.work_area = reserved;
                panel::apply_panel_config(&self.panel, &self.config, self.work_area);
                let message = format!("work area update: {:?}", self.work_area);
                self.log_debug(PanelDebugLevel::Info, move || message);
            }
            UiEvent::RefreshWidgets => {
                if self.panel_visible {
                    self.refresh_widgets(false);
                }
            }
            UiEvent::CssReload => {
                debug!("css reload requested");
                self.css.reload(css::DEFAULT_CSS);
                self.log_debug(PanelDebugLevel::Info, || "css reloaded".to_string());
            }
            UiEvent::ConfigReload => {
                debug!("config reload requested");
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
        let theme_paths = match config.resolve_theme_paths() {
            Ok(paths) => paths,
            Err(err) => {
                tracing::warn!(?err, "failed to resolve theme paths");
                return;
            }
        };

        self.config = config.clone();
        debug!("config reloaded");
        self.css.update_theme(theme_paths, config.theme.clone());
        self.css.reload(css::DEFAULT_CSS);
        panel::apply_panel_config(&self.panel, &config, self.work_area);
        self.log_debug(
            PanelDebugLevel::Info,
            || "panel config applied after reload".to_string(),
        );
        self.apply_media_config(&config);
        self.apply_widget_config(&config);
        self.restart_refresh_timer();
        if config.panel.respect_work_area {
            self.work_area = None;
            hyprland::refresh_reserved_work_area(config.panel.output.clone(), self.event_tx.clone());
        }
    }

    fn apply_media_config(&mut self, config: &Config) {
        if !config.media.enabled {
            self.panel.media_container.set_visible(false);
            self.clear_media_container();
            self.media = None;
            debug!("media disabled");
            return;
        }

        self.panel.media_container.set_visible(true);
        match (self.media.as_mut(), self.media_handle.as_ref()) {
            (Some(media), _) => {
                debug!("media layout updated");
                media.apply_layout(config.panel.width, config.media.title_char_limit);
            }
            (None, Some(handle)) => {
                debug!("media widget created");
                let media = media_widget::MediaWidget::new(
                    &self.panel.media_container,
                    handle.clone(),
                    config.panel.width,
                    config.media.title_char_limit,
                );
                self.media = Some(media);
            }
            (None, None) => {
                tracing::warn!("media runtime not available; restart required to enable media");
            }
        }
    }

    fn apply_widget_config(&mut self, config: &Config) {
        clear_container(&self.panel.quick_controls);
        let (volume, brightness) = build_quick_controls(&self.panel, config);
        self.volume = volume;
        self.brightness = brightness;
        clear_container(&self.panel.toggle_container);
        clear_container(&self.panel.stat_container);
        clear_container(&self.panel.card_container);
        let (toggles, stats, cards) = build_extra_widgets(&self.panel, config);
        self.toggles = toggles;
        self.stats = stats;
        self.cards = cards;
    }

    fn restart_refresh_timer(&mut self) {
        if self.panel_visible {
            self.stop_refresh_timer();
            self.start_refresh_timer();
        }
    }

    fn clear_media_container(&self) {
        while let Some(child) = self.panel.media_container.first_child() {
            self.panel.media_container.remove(&child);
        }
    }

    fn update_state(&mut self, state: unixnotis_core::ControlState) {
        self.dnd_guard.set(true);
        self.panel.dnd_toggle.set_active(state.dnd_enabled);
        self.dnd_guard.set(false);
    }

    fn refresh_counts(&self) {
        let total = self.list.total_count();
        self.panel.header_count.set_text(&format!("{total}"));
    }

    fn apply_panel_request(&mut self, request: PanelRequest) {
        match request.action {
            unixnotis_core::PanelAction::Open => {
                debug::set_level(PanelDebugLevel::Off);
                self.set_visible(true);
            }
            unixnotis_core::PanelAction::Close => {
                debug::set_level(PanelDebugLevel::Off);
                self.set_visible(false);
            }
            unixnotis_core::PanelAction::Toggle => {
                if !self.panel_visible {
                    debug::set_level(PanelDebugLevel::Off);
                }
                self.set_visible(!self.panel_visible);
            }
        }

        if request.debug != PanelDebugLevel::Off {
            debug::set_level(request.debug);
            self.log_debug(
                PanelDebugLevel::Info,
                || format!("debug mode enabled: {:?}", request.debug),
            );
        }
    }

    fn set_visible(&mut self, visible: bool) {
        self.panel_visible = visible;
        self.panel_visible_flag.store(visible, Ordering::SeqCst);
        self.panel.window.set_visible(visible);
        debug!(visible, "panel visibility updated");
        self.log_debug(
            PanelDebugLevel::Info,
            || format!("panel visibility set to {visible}"),
        );
        if visible {
            let width = self.panel.window.allocated_width();
            let height = self.panel.window.allocated_height();
            let message = format!("panel allocated size: {width}x{height}");
            self.log_debug(PanelDebugLevel::Verbose, move || message);
        }
        if visible {
            if let Some(volume) = self.volume.as_ref() {
                volume.set_watch_active(true);
            }
            if let Some(brightness) = self.brightness.as_ref() {
                brightness.set_watch_active(true);
            }
            if let Some(toggles) = self.toggles.as_ref() {
                toggles.set_watch_active(true);
            }
            self.panel.root.grab_focus();
            if let Some(handle) = self.media_handle.as_ref() {
                handle.refresh();
            }
            self.refresh_widgets(true);
            self.start_refresh_timer();
        } else {
            if let Some(volume) = self.volume.as_ref() {
                volume.set_watch_active(false);
            }
            if let Some(brightness) = self.brightness.as_ref() {
                brightness.set_watch_active(false);
            }
            if let Some(toggles) = self.toggles.as_ref() {
                toggles.set_watch_active(false);
            }
            self.stop_refresh_timer();
            debug::set_level(PanelDebugLevel::Off);
        }
    }

    fn close_if_click_outside(&self) {
        if !self.panel_visible {
            return;
        }
        if !self.is_click_outside_panel() {
            self.log_debug(
                PanelDebugLevel::Verbose,
                || "click outside ignored (pointer inside panel)".to_string(),
            );
            return;
        }
        // Close requests go through the daemon to keep control state consistent.
        self.log_debug(
            PanelDebugLevel::Info,
            || "click outside detected; requesting close".to_string(),
        );
        let _ = self.command_tx.send(UiCommand::ClosePanel);
    }

    fn refresh_widgets(&mut self, force: bool) {
        let now = Instant::now();
        let fast_ms = self.config.widgets.refresh_interval_ms;
        let slow_ms = self.config.widgets.refresh_interval_slow_ms;
        if debug::allows(PanelDebugLevel::Verbose) {
            info!(force, fast_ms, slow_ms, "widget refresh tick");
        }

        let refresh_fast = force
            || (fast_ms > 0
                && self
                    .last_fast_refresh
                    .map(|last| now.duration_since(last).as_millis() as u64 >= fast_ms)
                    .unwrap_or(true));
        if refresh_fast {
            if let Some(volume) = self.volume.as_ref() {
                if force || volume.needs_polling() {
                    volume.refresh();
                }
            }
            if let Some(brightness) = self.brightness.as_ref() {
                if force || brightness.needs_polling() {
                    brightness.refresh();
                }
            }
            self.last_fast_refresh = Some(now);
        }

        let refresh_slow = force
            || (slow_ms > 0
                && self
                    .last_slow_refresh
                    .map(|last| now.duration_since(last).as_millis() as u64 >= slow_ms)
                    .unwrap_or(true));
        if refresh_slow {
            if let Some(toggles) = self.toggles.as_ref() {
                if force || toggles.needs_polling() {
                    toggles.refresh();
                }
            }
            if let Some(stats) = self.stats.as_ref() {
                stats.refresh();
            }
            if let Some(cards) = self.cards.as_ref() {
                cards.refresh();
            }
            self.last_slow_refresh = Some(now);
        }
    }

    fn start_refresh_timer(&mut self) {
        if self.refresh_source.is_some() {
            return;
        }
        let volume_poll = self.volume.as_ref().map(|widget| widget.needs_polling()).unwrap_or(false);
        let brightness_poll =
            self.brightness.as_ref().map(|widget| widget.needs_polling()).unwrap_or(false);
        let toggles_poll = self.toggles.as_ref().map(|widget| widget.needs_polling()).unwrap_or(false);
        let stats_poll = self.stats.is_some();
        let cards_poll = self.cards.is_some();
        if !(volume_poll || brightness_poll || toggles_poll || stats_poll || cards_poll) {
            return;
        }
        let fast = self.config.widgets.refresh_interval_ms;
        let slow = self.config.widgets.refresh_interval_slow_ms;
        let interval = match (fast, slow) {
            (0, 0) => 0,
            (0, slow) => slow,
            (fast, 0) => fast,
            (fast, slow) => fast.min(slow),
        };
        if interval == 0 {
            return;
        }
        let event_tx = self.event_tx.clone();
        let id =
            gtk::glib::timeout_add_local(std::time::Duration::from_millis(interval), move || {
                let _ = event_tx.try_send(UiEvent::RefreshWidgets);
                gtk::glib::ControlFlow::Continue
            });
        self.refresh_source = Some(id);
        self.log_debug(
            PanelDebugLevel::Info,
            || format!("refresh timer started ({} ms)", interval),
        );
    }

    fn stop_refresh_timer(&mut self) {
        if let Some(id) = self.refresh_source.take() {
            id.remove();
        }
        self.last_fast_refresh = None;
        self.last_slow_refresh = None;
        self.log_debug(PanelDebugLevel::Info, || "refresh timer stopped".to_string());
    }

    fn log_debug(&self, level: PanelDebugLevel, message: impl FnOnce() -> String) {
        debug::log(level, message);
    }

    fn is_click_outside_panel(&self) -> bool {
        // Hyprland focus changes can be hover-driven; only close when a mouse button is down.
        let Some(display) = gdk::Display::default() else {
            self.log_debug(
                PanelDebugLevel::Verbose,
                || "click outside check skipped (no display)".to_string(),
            );
            return false;
        };
        let Some(seat) = display.default_seat() else {
            self.log_debug(
                PanelDebugLevel::Verbose,
                || "click outside check skipped (no seat)".to_string(),
            );
            return false;
        };
        let Some(pointer) = seat.pointer() else {
            self.log_debug(
                PanelDebugLevel::Verbose,
                || "click outside check skipped (no pointer)".to_string(),
            );
            return false;
        };
        let modifiers = pointer.modifier_state();
        let click_active = modifiers.contains(gdk::ModifierType::BUTTON1_MASK)
            || modifiers.contains(gdk::ModifierType::BUTTON2_MASK)
            || modifiers.contains(gdk::ModifierType::BUTTON3_MASK);
        if !click_active {
            self.log_debug(
                PanelDebugLevel::Verbose,
                || "click outside check skipped (no button pressed)".to_string(),
            );
            return false;
        }
        let (surface, _, _) = pointer.surface_at_position();
        let panel_surface = self.panel.window.surface();
        if let (Some(surface), Some(panel_surface)) = (surface, panel_surface) {
            if surface == panel_surface {
                self.log_debug(
                    PanelDebugLevel::Verbose,
                    || "click outside check ignored (surface matches panel)".to_string(),
                );
                return false;
            }
        }
        true
    }
}

fn build_quick_controls(
    panel: &panel::PanelWidgets,
    config: &Config,
) -> (
    Option<widgets::volume::VolumeWidget>,
    Option<widgets::brightness::BrightnessWidget>,
) {
    let mut has_widgets = false;
    let volume = if config.widgets.volume.enabled {
        let widget = widgets::volume::VolumeWidget::new(config.widgets.volume.clone());
        panel.quick_controls.append(widget.root());
        has_widgets = true;
        Some(widget)
    } else {
        None
    };

    let brightness = if config.widgets.brightness.enabled {
        let widget = widgets::brightness::BrightnessWidget::new(config.widgets.brightness.clone());
        panel.quick_controls.append(widget.root());
        has_widgets = true;
        Some(widget)
    } else {
        None
    };

    panel.quick_controls.set_visible(has_widgets);
    (volume, brightness)
}

fn build_extra_widgets(
    panel: &panel::PanelWidgets,
    config: &Config,
) -> (
    Option<widgets::toggles::ToggleGrid>,
    Option<widgets::stats::StatGrid>,
    Option<widgets::cards::CardGrid>,
) {
    let toggles = widgets::toggles::ToggleGrid::new(&config.widgets.toggles);
    if let Some(grid) = toggles.as_ref() {
        panel.toggle_container.set_visible(true);
        panel.toggle_container.append(grid.root());
    } else {
        panel.toggle_container.set_visible(false);
    }

    let stats = widgets::stats::StatGrid::new(&config.widgets.stats);
    if let Some(grid) = stats.as_ref() {
        panel.stat_container.set_visible(true);
        panel.stat_container.append(grid.root());
    } else {
        panel.stat_container.set_visible(false);
    }

    let cards = widgets::cards::CardGrid::new(&config.widgets.cards);
    if let Some(grid) = cards.as_ref() {
        panel.card_container.set_visible(true);
        panel.card_container.append(grid.root());
    } else {
        panel.card_container.set_visible(false);
    }

    (toggles, stats, cards)
}

fn clear_container(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}
