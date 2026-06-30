use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::{
    notification_signal_mode_for_sender, NotificationBurstState, NotificationSignalMode,
    NOTIFICATION_DIRECT_SIGNAL_LIMIT, NOTIFICATION_SIGNAL_TRACK_LIMIT, NOTIFICATION_SIGNAL_WINDOW,
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

    // One snapshot tells clients to resync without flooding the bus
    assert_eq!(
        notification_signal_mode_for_sender(&cache, ":1.55"),
        NotificationSignalMode::SnapshotOnly
    );
    // Further events inside the same burst window are redundant
    assert_eq!(
        notification_signal_mode_for_sender(&cache, ":1.55"),
        NotificationSignalMode::Suppress
    );
}

#[test]
fn notification_signal_mode_caps_unique_senders_without_blocking_known_sender() {
    let now = Instant::now();
    let mut seeded = HashMap::<String, NotificationBurstState>::new();
    for index in 0..NOTIFICATION_SIGNAL_TRACK_LIMIT {
        seeded.insert(
            format!(":1.{index}"),
            NotificationBurstState {
                window_started: now,
                last_seen: now,
                count: 1,
                snapshot_emitted: false,
            },
        );
    }
    let cache = Mutex::new(seeded);

    // Unknown senders over the cap get a coarse snapshot instead of growing memory
    assert_eq!(
        notification_signal_mode_for_sender(&cache, ":1.new"),
        NotificationSignalMode::SnapshotOnly
    );
    // Known senders still get precise direct signals while inside their limit
    assert_eq!(
        notification_signal_mode_for_sender(&cache, ":1.7"),
        NotificationSignalMode::Direct
    );
}

#[test]
fn notification_signal_mode_prunes_expired_senders_before_track_limit_check() {
    let stale = Instant::now() - NOTIFICATION_SIGNAL_WINDOW - Duration::from_millis(1);
    let mut seeded = HashMap::<String, NotificationBurstState>::new();
    for index in 0..NOTIFICATION_SIGNAL_TRACK_LIMIT {
        seeded.insert(
            format!(":1.{index}"),
            NotificationBurstState {
                window_started: stale,
                last_seen: stale,
                count: 1,
                snapshot_emitted: false,
            },
        );
    }
    let cache = Mutex::new(seeded);

    // Stale entries should not make a new sender look like a tracking flood
    assert_eq!(
        notification_signal_mode_for_sender(&cache, ":1.fresh"),
        NotificationSignalMode::Direct
    );
}

#[test]
fn notification_signal_mode_resets_existing_sender_after_window_expires() {
    let now = Instant::now();
    let mut seeded = HashMap::<String, NotificationBurstState>::new();
    seeded.insert(
        ":1.noisy".to_string(),
        NotificationBurstState {
            window_started: now - NOTIFICATION_SIGNAL_WINDOW - Duration::from_millis(1),
            // Recent last_seen keeps the sender in the map so only the per-sender window resets
            last_seen: now,
            count: NOTIFICATION_DIRECT_SIGNAL_LIMIT + 1,
            snapshot_emitted: true,
        },
    );
    let cache = Mutex::new(seeded);

    // Expired windows regain the direct-signal allowance for normal later activity
    assert_eq!(
        notification_signal_mode_for_sender(&cache, ":1.noisy"),
        NotificationSignalMode::Direct
    );
}
