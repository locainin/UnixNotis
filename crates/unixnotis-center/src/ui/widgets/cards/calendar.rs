//! Calendar-specific refresh helpers

use std::time::{Duration, Instant};

use gtk::glib;
use tracing::warn;

use super::CardItem;

impl CardItem {
    pub(super) fn refresh_calendar(&self, base_interval: Duration) {
        let Some(calendar) = self.calendar.as_ref() else {
            return;
        };
        match glib::DateTime::now_local() {
            Ok(now) => {
                let date_key = (now.year(), now.month(), now.day_of_month());
                let changed = self.last_calendar_day.get() != Some(date_key);
                if changed {
                    // Day changes are the only time the GTK calendar needs a new selection
                    calendar.select_day(&now);
                    self.last_calendar_day.set(Some(date_key));
                }
                let delay = next_local_midnight_delay(&now)
                    .unwrap_or_else(|| base_interval.max(Duration::from_secs(60)));
                // Store an absolute instant so the scheduler can use one-shot wakeups
                self.calendar_next_due.set(Some(Instant::now() + delay));
            }
            Err(err) => {
                warn!(?err, "calendar refresh failed");
                self.calendar_next_due.set(Some(
                    Instant::now() + base_interval.max(Duration::from_secs(30)),
                ));
            }
        }
    }
}

pub(super) fn next_local_midnight_delay(now: &glib::DateTime) -> Option<Duration> {
    let next_day = now.add_days(1).ok()?;
    let timezone = glib::TimeZone::local();
    let midnight = glib::DateTime::new(
        &timezone,
        next_day.year(),
        next_day.month(),
        next_day.day_of_month(),
        0,
        0,
        0.0,
    )
    .ok()?;
    let now_unix = now.to_unix();
    let midnight_unix = midnight.to_unix();
    let seconds = midnight_unix.checked_sub(now_unix)?;
    if seconds <= 0 {
        return Some(Duration::from_secs(60));
    }
    Some(Duration::from_secs(seconds as u64))
}
