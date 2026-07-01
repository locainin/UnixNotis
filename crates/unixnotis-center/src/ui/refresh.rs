//! Widget refresh scheduling for `UiState`.
//!
//! Maintains deadline-based polling cadence and GLib timer lifecycle.

use std::time::{Duration, Instant};

use gtk::glib;
use tracing::info;
use unixnotis_core::PanelDebugLevel;

use crate::dbus::UiEvent;
use crate::debug;

use super::UiState;

impl UiState {
    pub(super) fn refresh_widgets(&mut self, force: bool) {
        // Global refresh entry counter helps compare open vs settled churn
        super::perf_probe::refresh_widgets_called();
        let now = Instant::now();
        let fast_ms = self.config.widgets.refresh_interval_ms;
        let slow_ms = self.config.widgets.refresh_interval_slow_ms;
        if debug::allows(PanelDebugLevel::Verbose) {
            info!(force, fast_ms, slow_ms, "widget refresh tick");
        }

        // Fast controls expose per-widget deadlines so stable sliders do not
        // keep the whole panel on a one-second wakeup loop.
        let fast_base = Duration::from_millis(fast_ms.max(1));
        let volume_due = self
            .volume
            .as_ref()
            .and_then(|widget| widget.next_poll_in(now, fast_base))
            .map(is_due_delay)
            .unwrap_or(false);
        let brightness_due = self
            .brightness
            .as_ref()
            .and_then(|widget| widget.next_poll_in(now, fast_base))
            .map(is_due_delay)
            .unwrap_or(false);
        let fast_due = force || (fast_ms > 0 && (volume_due || brightness_due));
        if fast_due {
            super::perf_probe::refresh_fast_lane_due();
            if let Some(volume) = self.volume.as_ref() {
                if force || volume_due {
                    super::perf_probe::refresh_volume_called();
                    volume.refresh(fast_base, force);
                }
            }
            if let Some(brightness) = self.brightness.as_ref() {
                if force || brightness_due {
                    super::perf_probe::refresh_brightness_called();
                    brightness.refresh(fast_base, force);
                }
            }
        }

        // Slow cadence is reserved for less dynamic controls to keep idle CPU low.
        let toggles_due = force
            || interval_due(now, self.last_slow_refresh, slow_ms)
                .map(is_due_delay)
                .unwrap_or(false);
        if toggles_due {
            super::perf_probe::refresh_slow_lane_due();
            if let Some(toggles) = self.toggles.as_ref() {
                if force || toggles.needs_polling() {
                    super::perf_probe::refresh_toggles_called();
                    toggles.refresh();
                }
            }
            self.last_slow_refresh = Some(now);
        }

        // Widget-level backoff logic scales from this baseline.
        let slow_base = Duration::from_millis(slow_ms.max(1));
        if let Some(stats) = self.stats.as_ref() {
            if force || stats.is_due(now) {
                super::perf_probe::refresh_stats_called();
                stats.refresh(slow_base, force);
            }
        }
        if let Some(cards) = self.cards.as_ref() {
            if force || cards.is_due(now) {
                super::perf_probe::refresh_cards_called();
                cards.refresh(slow_base, force);
            }
        }
    }

    pub(super) fn start_refresh_timer(&mut self) {
        if self.refresh_source.is_some() {
            return;
        }
        let now = Instant::now();
        // Arm a one-shot timer for the nearest widget deadline rather than
        // running a fixed periodic tick.
        let Some(mut delay) = self.next_refresh_delay(now) else {
            return;
        };
        // GLib timeout granularity is millisecond-based
        // Any sub-millisecond delay can collapse into immediate re-fire churn
        if delay < Duration::from_millis(1) {
            delay = Duration::from_millis(1);
        }
        let event_tx = self.event_tx.clone();
        let id = glib::timeout_add_local_once(delay, move || {
            let _ = event_tx.try_send(UiEvent::RefreshWidgets);
        });
        self.refresh_source = Some(id);
        super::perf_probe::refresh_timer_armed();
        self.log_debug(PanelDebugLevel::Info, move || {
            format!("refresh timer armed for {} ms", delay.as_millis())
        });
    }

    fn next_refresh_delay(&self, now: Instant) -> Option<Duration> {
        let mut next = None;
        if self.config.widgets.refresh_interval_ms > 0 {
            let fast_base = Duration::from_millis(self.config.widgets.refresh_interval_ms.max(1));
            if let Some(volume) = self.volume.as_ref() {
                update_next_delay(&mut next, volume.next_poll_in(now, fast_base));
            }
            if let Some(brightness) = self.brightness.as_ref() {
                update_next_delay(&mut next, brightness.next_poll_in(now, fast_base));
            }
        }

        let toggles_poll = self
            .toggles
            .as_ref()
            .map(|widget| widget.needs_polling())
            .unwrap_or(false);
        if toggles_poll {
            // Slow lane uses the configured slower interval by default.
            update_next_delay(
                &mut next,
                interval_due(
                    now,
                    self.last_slow_refresh,
                    self.config.widgets.refresh_interval_slow_ms,
                ),
            );
        }

        if let Some(stats) = self.stats.as_ref() {
            // Stats/cards expose their own next deadline, including backoff.
            update_next_delay(&mut next, stats.next_refresh_in(now));
        }
        if let Some(cards) = self.cards.as_ref() {
            update_next_delay(&mut next, cards.next_refresh_in(now));
        }
        next
    }

    pub(super) fn stop_refresh_timer(&mut self) {
        if let Some(id) = self.refresh_source.take() {
            id.remove();
        }
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

fn interval_due(now: Instant, last: Option<Instant>, interval_ms: u64) -> Option<Duration> {
    if interval_ms == 0 {
        // Zero disables this interval lane completely.
        return None;
    }
    let base = Duration::from_millis(interval_ms);
    Some(match last {
        Some(last) => base.saturating_sub(now.saturating_duration_since(last)),
        None => Duration::ZERO,
    })
}

fn update_next_delay(next: &mut Option<Duration>, candidate: Option<Duration>) {
    let Some(candidate) = candidate else {
        return;
    };
    match next {
        // Keep the earliest non-empty candidate as the next one-shot wakeup.
        Some(current) if candidate >= *current => {}
        _ => *next = Some(candidate),
    }
}

fn is_due_delay(delay: Duration) -> bool {
    // Treat sub-millisecond jitter as due so one-shot scheduling does not spin
    delay <= Duration::from_millis(1)
}

#[cfg(test)]
#[path = "tests/refresh.rs"]
mod tests;
