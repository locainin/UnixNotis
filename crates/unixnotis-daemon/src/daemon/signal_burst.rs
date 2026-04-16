//! Burst tracking for notification fanout

use std::collections::HashMap;
use std::sync::Mutex as StdMutex;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NotificationSignalMode {
    Direct,
    SnapshotOnly,
    Suppress,
}

#[derive(Clone, Debug)]
pub(super) struct NotificationBurstState {
    window_started: Instant,
    last_seen: Instant,
    count: u16,
    snapshot_emitted: bool,
}

const NOTIFICATION_SIGNAL_WINDOW: Duration = Duration::from_secs(1);
const NOTIFICATION_DIRECT_SIGNAL_LIMIT: u16 = 8;
const NOTIFICATION_SIGNAL_TRACK_LIMIT: usize = 128;

pub(super) fn notification_signal_mode_for_sender(
    cache: &StdMutex<HashMap<String, NotificationBurstState>>,
    sender: &str,
) -> NotificationSignalMode {
    let now = Instant::now();
    let mut cache = match cache.lock() {
        Ok(cache) => cache,
        Err(poisoned) => poisoned.into_inner(),
    };

    // Old senders fall out once their burst window expires
    cache.retain(|_, state| now.duration_since(state.last_seen) <= NOTIFICATION_SIGNAL_WINDOW);
    if cache.len() >= NOTIFICATION_SIGNAL_TRACK_LIMIT && !cache.contains_key(sender) {
        // Unknown senders beyond the small tracking cap fall back to snapshot mode
        return NotificationSignalMode::SnapshotOnly;
    }

    let state = cache
        .entry(sender.to_string())
        .or_insert_with(|| NotificationBurstState {
            window_started: now,
            last_seen: now,
            count: 0,
            snapshot_emitted: false,
        });

    // A fresh window resets the direct-signal allowance for that sender
    if now.duration_since(state.window_started) > NOTIFICATION_SIGNAL_WINDOW {
        state.window_started = now;
        state.count = 0;
        state.snapshot_emitted = false;
    }
    state.last_seen = now;
    state.count = state.count.saturating_add(1);

    if state.count <= NOTIFICATION_DIRECT_SIGNAL_LIMIT {
        return NotificationSignalMode::Direct;
    }
    if !state.snapshot_emitted {
        // One snapshot invalidation tells trusted UIs to resync once without replaying the whole burst
        state.snapshot_emitted = true;
        return NotificationSignalMode::SnapshotOnly;
    }
    // Extra events inside the same burst window add no value once the snapshot refresh is queued
    NotificationSignalMode::Suppress
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use super::{
        notification_signal_mode_for_sender, NotificationBurstState, NotificationSignalMode,
        NOTIFICATION_DIRECT_SIGNAL_LIMIT,
    };

    #[test]
    fn notification_signal_mode_falls_back_after_burst_limit() {
        let cache = Mutex::new(HashMap::<String, NotificationBurstState>::new());

        for _ in 0..NOTIFICATION_DIRECT_SIGNAL_LIMIT {
            assert_eq!(
                notification_signal_mode_for_sender(&cache, ":1.55"),
                NotificationSignalMode::Direct
            );
        }
        assert_eq!(
            notification_signal_mode_for_sender(&cache, ":1.55"),
            NotificationSignalMode::SnapshotOnly
        );
        assert_eq!(
            notification_signal_mode_for_sender(&cache, ":1.55"),
            NotificationSignalMode::Suppress
        );
    }
}
