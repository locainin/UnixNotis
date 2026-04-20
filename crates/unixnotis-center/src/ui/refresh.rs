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

        // Fast cadence is used for controls that can visibly drift quickly
        // (for example volume/backlight values).
        let fast_due = force
            || interval_due(now, self.last_fast_refresh, fast_ms)
                .map(is_due_delay)
                .unwrap_or(false);
        if fast_due {
            super::perf_probe::refresh_fast_lane_due();
            if let Some(volume) = self.volume.as_ref() {
                if force || volume.needs_polling() {
                    super::perf_probe::refresh_volume_called();
                    volume.refresh();
                }
            }
            if let Some(brightness) = self.brightness.as_ref() {
                if force || brightness.needs_polling() {
                    super::perf_probe::refresh_brightness_called();
                    brightness.refresh();
                }
            }
            self.last_fast_refresh = Some(now);
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
        if volume_poll || brightness_poll {
            // Fast lane only participates when at least one source is polling.
            update_next_delay(
                &mut next,
                interval_due(
                    now,
                    self.last_fast_refresh,
                    self.config.widgets.refresh_interval_ms,
                ),
            );
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
mod tests {
    use super::update_next_delay;
    use std::time::Duration;

    #[test]
    fn deadline_scheduler_supports_lower_idle_wakeups_than_fixed_ticks() {
        // Legacy slow polling wakes every 3s with defaults (20 wakeups/minute).
        let legacy_wakeups_per_min = 60.0 / 3.0;

        // Deadline model uses each widget's own due time. Stable stats at 12s and
        // calendar at daily cadence produce a 12s next wakeup (5 wakeups/minute).
        let mut next = None;
        update_next_delay(&mut next, Some(Duration::from_secs(12)));
        update_next_delay(&mut next, Some(Duration::from_secs(24 * 60 * 60)));
        let delay = next.expect("next deadline");
        let deadline_wakeups_per_min = 60.0 / delay.as_secs_f64();

        assert!(deadline_wakeups_per_min < legacy_wakeups_per_min);
        assert_eq!(delay, Duration::from_secs(12));
    }
}
