//! Widget refresh scheduling for `UiState`.
//!
//! Maintains the fast/slow polling cadence and the GLib timer lifecycle.

use std::time::{Duration, Instant};

use gtk::glib;
use tracing::info;
use unixnotis_core::PanelDebugLevel;

use crate::dbus::UiEvent;
use crate::debug;

use super::UiState;

impl UiState {
    pub(super) fn refresh_widgets(&mut self, force: bool) {
        let now = Instant::now();
        let fast_ms = self.config.widgets.refresh_interval_ms;
        let slow_ms = self.config.widgets.refresh_interval_slow_ms;
        if debug::allows(PanelDebugLevel::Verbose) {
            info!(force, fast_ms, slow_ms, "widget refresh tick");
        }

        // Fast refresh covers high-frequency widgets like volume/brightness.
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

        // Slow refresh covers less frequent widgets like toggles/stats/cards.
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
                stats.refresh(Duration::from_millis(slow_ms), force);
            }
            if let Some(cards) = self.cards.as_ref() {
                cards.refresh(Duration::from_millis(slow_ms), force);
            }
            self.last_slow_refresh = Some(now);
        }
    }

    pub(super) fn start_refresh_timer(&mut self) {
        if self.refresh_source.is_some() {
            return;
        }
        let volume_poll = self
            .volume
            .as_ref()
            .map(|widget| widget.needs_polling())
            .unwrap_or(false);
        let brightness_poll = self
            .brightness
            .as_ref()
            .map(|widget| widget.needs_polling())
            .unwrap_or(false);
        let toggles_poll = self
            .toggles
            .as_ref()
            .map(|widget| widget.needs_polling())
            .unwrap_or(false);
        let stats_poll = self.stats.is_some();
        let cards_poll = self.cards.is_some();
        if !(volume_poll || brightness_poll || toggles_poll || stats_poll || cards_poll) {
            // Skip timer creation when no widgets require periodic work.
            return;
        }
        let fast = self.config.widgets.refresh_interval_ms;
        let slow = self.config.widgets.refresh_interval_slow_ms;
        // Prefer the fastest interval only when fast-refresh widgets are active.
        let fast_interval = if volume_poll || brightness_poll {
            if fast > 0 {
                Some(fast)
            } else {
                None
            }
        } else {
            None
        };
        let slow_interval = if toggles_poll || stats_poll || cards_poll {
            if slow > 0 {
                Some(slow)
            } else {
                None
            }
        } else {
            None
        };
        let interval = match (fast_interval, slow_interval) {
            (Some(fast), Some(slow)) => fast.min(slow),
            (Some(fast), None) => fast,
            (None, Some(slow)) => slow,
            (None, None) => 0,
        };
        if interval == 0 {
            // Both refresh intervals disabled; avoid scheduling a timer.
            return;
        }
        let event_tx = self.event_tx.clone();
        let id = glib::timeout_add_local(std::time::Duration::from_millis(interval), move || {
            let _ = event_tx.try_send(UiEvent::RefreshWidgets);
            glib::ControlFlow::Continue
        });
        self.refresh_source = Some(id);
        self.log_debug(PanelDebugLevel::Info, || {
            format!("refresh timer started ({} ms)", interval)
        });
    }

    pub(super) fn stop_refresh_timer(&mut self) {
        if let Some(id) = self.refresh_source.take() {
            id.remove();
        }
        self.last_fast_refresh = None;
        self.last_slow_refresh = None;
        self.log_debug(PanelDebugLevel::Info, || {
            "refresh timer stopped".to_string()
        });
    }

    pub(super) fn restart_refresh_timer(&mut self) {
        if self.panel_visible {
            self.stop_refresh_timer();
            self.start_refresh_timer();
        }
    }
}
