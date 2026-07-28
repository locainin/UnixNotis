//! Burst tracking for notification fanout

use std::collections::HashMap;
use std::sync::Mutex as StdMutex;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NotificationSignalMode {
    // Normal path: send the precise notification signal
    Direct,
    // Burst path: send invalidations so clients can rebuild from committed state
    SnapshotOnly,
}

#[derive(Clone, Debug)]
pub(in crate::daemon) struct NotificationBurstState {
    window_started: Instant,
    last_seen: Instant,
    count: u16,
}

const NOTIFICATION_SIGNAL_WINDOW: Duration = Duration::from_secs(1);
// Keep a small direct burst so ordinary app batches still feel immediate
const NOTIFICATION_DIRECT_SIGNAL_LIMIT: u16 = 8;
// Cap tracked senders so hostile unique names cannot grow memory without bound
const NOTIFICATION_SIGNAL_TRACK_LIMIT: usize = 128;

pub(in crate::daemon) fn notification_signal_mode_for_sender(
    cache: &StdMutex<HashMap<String, NotificationBurstState>>,
    sender: &str,
) -> NotificationSignalMode {
    notification_signal_mode_for_sender_at(cache, sender, Instant::now())
}

fn notification_signal_mode_for_sender_at(
    cache: &StdMutex<HashMap<String, NotificationBurstState>>,
    sender: &str,
    now: Instant,
) -> NotificationSignalMode {
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
        });

    // A fresh window resets the direct-signal allowance for that sender
    if now.duration_since(state.window_started) >= NOTIFICATION_SIGNAL_WINDOW {
        state.window_started = now;
        state.count = 0;
    }
    state.last_seen = now;
    state.count = state.count.saturating_add(1);

    if state.count <= NOTIFICATION_DIRECT_SIGNAL_LIMIT {
        return NotificationSignalMode::Direct;
    }
    // Every trailing commit invalidates the prior snapshot because its fetch may already be running
    NotificationSignalMode::SnapshotOnly
}

#[cfg(test)]
#[path = "tests/flow_control.rs"]
mod tests;
