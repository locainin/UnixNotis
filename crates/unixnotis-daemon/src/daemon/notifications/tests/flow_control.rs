use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::{
    notification_signal_mode_for_sender, notification_signal_mode_for_sender_at,
    NotificationBurstState, NotificationSignalMode, NOTIFICATION_DIRECT_SIGNAL_LIMIT,
    NOTIFICATION_SIGNAL_TRACK_LIMIT, NOTIFICATION_SIGNAL_WINDOW,
};

#[test]
fn notification_signal_mode_invalidates_every_trailing_burst_commit() {
    let cache = Mutex::new(HashMap::<String, NotificationBurstState>::new());

    for _ in 0..NOTIFICATION_DIRECT_SIGNAL_LIMIT {
        assert_eq!(
            notification_signal_mode_for_sender(&cache, ":1.55"),
            NotificationSignalMode::Direct
        );
    }

    // The first overflow switches clients to the bounded snapshot path
    assert_eq!(
        notification_signal_mode_for_sender(&cache, ":1.55"),
        NotificationSignalMode::SnapshotOnly
    );
    // Later commits need their own invalidation because the first fetch may already be in flight
    for _ in 0..3 {
        assert_eq!(
            notification_signal_mode_for_sender(&cache, ":1.55"),
            NotificationSignalMode::SnapshotOnly
        );
    }
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
    let stale = Instant::now()
        .checked_sub(NOTIFICATION_SIGNAL_WINDOW + Duration::from_millis(1))
        .expect("test clock should represent the previous burst window");
    let mut seeded = HashMap::<String, NotificationBurstState>::new();
    for index in 0..NOTIFICATION_SIGNAL_TRACK_LIMIT {
        seeded.insert(
            format!(":1.{index}"),
            NotificationBurstState {
                window_started: stale,
                last_seen: stale,
                count: 1,
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
            window_started: now
                .checked_sub(NOTIFICATION_SIGNAL_WINDOW + Duration::from_millis(1))
                .expect("test clock should represent the previous burst window"),
            // Recent last_seen keeps the sender in the map so only the per-sender window resets
            last_seen: now,
            count: NOTIFICATION_DIRECT_SIGNAL_LIMIT + 1,
        },
    );
    let cache = Mutex::new(seeded);

    // Expired windows regain the direct-signal allowance for normal later activity
    assert_eq!(
        notification_signal_mode_for_sender(&cache, ":1.noisy"),
        NotificationSignalMode::Direct
    );
}

#[test]
fn notification_signal_mode_resets_at_the_exact_window_boundary() {
    let now = Instant::now();
    let window_started = now
        .checked_sub(NOTIFICATION_SIGNAL_WINDOW)
        .expect("test clock should represent the burst boundary");
    let cache = Mutex::new(HashMap::from([(
        ":1.boundary".to_string(),
        NotificationBurstState {
            window_started,
            last_seen: now,
            count: NOTIFICATION_DIRECT_SIGNAL_LIMIT + 1,
        },
    )]));

    assert_eq!(
        notification_signal_mode_for_sender_at(&cache, ":1.boundary", now),
        NotificationSignalMode::Direct
    );
}
