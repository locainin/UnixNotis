//! Store regression coverage and persistence validation.

use super::store_state::{PersistedDndState, DND_STATE_FILE, DND_STATE_VERSION};
use super::{contains_ci, NotificationStore};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use unixnotis_core::{CloseReason, Config, InhibitMode, Notification, NotificationImage, Urgency};
use zbus::zvariant::OwnedValue;

#[test]
fn contains_ci_matches_ascii() {
    assert!(contains_ci("Signal-Desktop", "signal"));
    assert!(contains_ci("signal-desktop", "Signal"));
    assert!(!contains_ci("signal-desktop", "brave"));
    assert!(contains_ci("mixedCase", "case"));
    assert!(contains_ci("mixedCase", ""));
}

fn make_notification(summary: &str) -> Notification {
    Notification {
        id: 0,
        app_name: "TestApp".to_string(),
        app_icon: String::new(),
        summary: summary.to_string(),
        body: String::new(),
        actions: Vec::new(),
        hints: HashMap::<String, OwnedValue>::new(),
        urgency: Urgency::Normal,
        category: None,
        is_transient: false,
        is_resident: false,
        suppress_popup: false,
        suppress_sound: false,
        image: NotificationImage::default(),
        expire_timeout: 0,
        received_at: Utc::now(),
        sender_name: Some(":1.test".to_string()),
        sender_pid: Some(1234),
        sender_start_time: Some(555),
        sender_executable: Some("/usr/bin/test-app".to_string()),
    }
}

fn make_notification_with_sender(
    summary: &str,
    sender: &str,
    pid: u32,
    start_time: u64,
) -> Notification {
    let mut notification = make_notification(summary);
    notification.sender_name = Some(sender.to_string());
    notification.sender_pid = Some(pid);
    notification.sender_start_time = Some(start_time);
    notification
}

fn make_store_with_limits(max_active: usize, max_entries: usize) -> NotificationStore {
    let mut config = Config::default();
    // Test helper uses explicit limits so each case isolates one policy branch
    config.history.max_active = max_active;
    config.history.max_entries = max_entries;
    NotificationStore::new(config)
}

fn make_temp_state_dir(label: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    path.push(format!("unixnotis-test-{label}-{pid}-{nanos}"));
    std::fs::create_dir_all(&path).expect("create temp state dir");
    path
}

fn write_dnd_state(dir: &std::path::Path, enabled: bool, version: u32) {
    let state = PersistedDndState {
        version,
        dnd_enabled: enabled,
        updated_at: Some("2025-01-01T00:00:00Z".to_string()),
    };
    let payload = serde_json::to_string(&state).expect("serialize state");
    let path = dir.join("unixnotis").join(DND_STATE_FILE);
    std::fs::create_dir_all(path.parent().expect("state parent")).expect("create state directory");
    std::fs::write(&path, payload).expect("write state");
}

fn cleanup_temp_dir(dir: &std::path::Path) {
    let _ = std::fs::remove_dir_all(dir);
}

fn apply_dnd_update(store: &mut NotificationStore, enabled: bool) -> bool {
    let write = store.set_dnd(enabled);
    if let Some(state_store) = write.persist.as_ref() {
        state_store
            .persist(write.current)
            .expect("persist dnd state");
    }
    write.changed
}

#[test]
fn max_active_zero_archives_immediately() {
    let mut store = make_store_with_limits(0, 10);

    let outcome = store.insert(make_notification("first"), 0);
    assert_eq!(outcome.evicted.len(), 1);
    assert!(store.list_active().is_empty());
    assert_eq!(store.history_len(), 1);

    store.insert(make_notification("second"), 0);
    assert!(store.list_active().is_empty());
    assert_eq!(store.history_len(), 2);
}

#[test]
fn max_active_evicts_oldest_to_history() {
    let mut store = make_store_with_limits(1, 10);

    store.insert(make_notification("first"), 0);
    let outcome = store.insert(make_notification("second"), 0);

    assert_eq!(outcome.evicted.len(), 1);
    let active = store.list_active();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].summary, "second");
    assert_eq!(store.history_len(), 1);
}

#[test]
fn max_active_hard_cap_limits_even_when_config_is_higher() {
    // Config may request a larger active window, but runtime hard-cap protects UI stability.
    let mut store = make_store_with_limits(32, 64);

    for idx in 0..18 {
        // Insert in-order so expected active/history boundaries are easy to assert
        store.insert(make_notification(&format!("entry-{idx}")), 0);
    }

    let active = store.list_active();
    let history = store.list_history();

    assert_eq!(active.len(), 12);
    assert_eq!(history.len(), 6);
    // Newest remains at front after cap-based eviction
    assert_eq!(active[0].summary, "entry-17");
    // Oldest retained active entry starts where cap boundary begins
    assert_eq!(active[11].summary, "entry-6");
}

#[test]
fn max_entries_zero_drops_history_on_close() {
    let mut store = make_store_with_limits(10, 0);

    let outcome = store.insert(make_notification("first"), 0);
    store.close(outcome.notification.id, CloseReason::Expired);

    assert_eq!(store.history_len(), 0);
}

#[test]
fn replace_id_in_history_reuses_id_and_clears_entry() {
    let mut store = make_store_with_limits(2, 10);

    let first = store.insert(make_notification("first"), 0);
    store.close(first.notification.id, CloseReason::Expired);
    assert_eq!(store.history_len(), 1);

    // Replacement should reuse the original ID and remove the history entry.
    let replaced = store.insert(make_notification("replacement"), first.notification.id);
    assert!(replaced.replaced);
    assert_eq!(replaced.notification.id, first.notification.id);
    assert_eq!(store.history_len(), 0);

    let active = store.list_active();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].summary, "replacement");

    // Closing the replacement should re-add a single history entry for the updated notification.
    store.close(replaced.notification.id, CloseReason::Expired);
    let history = store.list_history();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].summary, "replacement");
}

#[test]
fn replace_id_rejected_for_different_sender() {
    let mut store = make_store_with_limits(2, 10);

    let first = store.insert(
        make_notification_with_sender("first", ":1.sender-a", 101, 1),
        0,
    );
    store.close(first.notification.id, CloseReason::Expired);
    assert_eq!(store.history_len(), 1);

    // Cross-sender replacement must allocate a fresh id and keep prior history intact.
    let replaced = store.insert(
        make_notification_with_sender("replacement", ":1.sender-b", 202, 2),
        first.notification.id,
    );
    assert!(!replaced.replaced);
    assert_ne!(replaced.notification.id, first.notification.id);
    assert_eq!(store.history_len(), 1);
}

#[test]
fn history_eviction_keeps_most_recent_entries() {
    let mut store = make_store_with_limits(0, 2);

    store.insert(make_notification("first"), 0);
    store.insert(make_notification("second"), 0);
    store.insert(make_notification("third"), 0);

    // History listing returns most-recent-first order.
    let history = store.list_history();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].summary, "third");
    assert_eq!(history[1].summary, "second");
}

#[test]
fn max_entries_zero_drops_history_on_insert() {
    let mut store = make_store_with_limits(0, 0);

    let outcome = store.insert(make_notification("first"), 0);

    // Eviction should archive the active entry, then drop it due to the zero history limit.
    assert_eq!(outcome.evicted.len(), 1);
    assert!(store.list_active().is_empty());
    assert_eq!(store.history_len(), 0);
}

#[test]
fn transient_close_skips_history_when_config_disables_it() {
    let mut config = Config::default();
    // This case is the policy that the center must mirror exactly
    config.history.transient_to_history = false;
    let mut store = NotificationStore::new(config);

    let mut notification = make_notification("transient");
    notification.is_transient = true;
    let outcome = store.insert(notification, 0);
    store.close(outcome.notification.id, CloseReason::Expired);

    assert_eq!(store.history_len(), 0);
}

#[test]
fn transient_close_archives_when_config_allows_it() {
    let mut config = Config::default();
    // Explicit opt-in should keep the closed row in history
    config.history.transient_to_history = true;
    let mut store = NotificationStore::new(config);

    let mut notification = make_notification("transient");
    notification.is_transient = true;
    let outcome = store.insert(notification, 0);
    store.close(outcome.notification.id, CloseReason::Expired);

    assert_eq!(store.history_len(), 1);
}

#[test]
fn dnd_state_overrides_default() {
    let state_dir = make_temp_state_dir("dnd-override");
    write_dnd_state(&state_dir, true, DND_STATE_VERSION);

    let mut config = Config::default();
    config.general.dnd_default = false;
    let store = NotificationStore::new_with_state_dir(config, state_dir.clone());
    assert!(store.dnd_enabled());

    cleanup_temp_dir(&state_dir);
}

#[test]
fn dnd_state_invalid_payload_falls_back_to_default() {
    let state_dir = make_temp_state_dir("dnd-invalid");
    let path = state_dir.join("unixnotis").join(DND_STATE_FILE);
    std::fs::create_dir_all(path.parent().expect("state parent")).expect("create state directory");
    std::fs::write(&path, "{").expect("write invalid state");

    let mut config = Config::default();
    config.general.dnd_default = true;
    let store = NotificationStore::new_with_state_dir(config, state_dir.clone());
    assert!(store.dnd_enabled());

    cleanup_temp_dir(&state_dir);
}

#[test]
fn dnd_state_persists_on_change() {
    let state_dir = make_temp_state_dir("dnd-write");
    let mut config = Config::default();
    config.general.dnd_default = false;
    let mut store = NotificationStore::new_with_state_dir(config, state_dir.clone());
    assert!(apply_dnd_update(&mut store, true));

    let path = state_dir.join("unixnotis").join(DND_STATE_FILE);
    let contents = std::fs::read_to_string(&path).expect("read persisted state");
    let parsed: PersistedDndState = serde_json::from_str(&contents).expect("parse persisted state");
    assert!(parsed.dnd_enabled);

    cleanup_temp_dir(&state_dir);
}

#[test]
fn dnd_toggle_flips_state_in_one_store_mutation() {
    let mut config = Config::default();
    config.general.dnd_default = false;
    let mut store = NotificationStore::new(config);

    let first = store.toggle_dnd();
    assert!(first.changed);
    assert!(!first.previous);
    assert!(first.current);
    assert!(store.dnd_enabled());

    let second = store.toggle_dnd();
    assert!(second.changed);
    assert!(second.previous);
    assert!(!second.current);
    assert!(!store.dnd_enabled());
}

#[test]
fn stale_dnd_rollback_cannot_overwrite_newer_write() {
    let mut config = Config::default();
    config.general.dnd_default = false;
    let mut store = NotificationStore::new(config);

    let write_a = store.set_dnd(true);
    assert!(store.dnd_enabled());

    let write_b = store.set_dnd(false);
    assert!(write_b.changed);
    assert!(!store.dnd_enabled());

    // Simulate late failure from write_a and verify guarded rollback is rejected.
    let rolled_back = store.rollback_dnd_write_if_current(&write_a);
    assert!(!rolled_back);
    assert!(!store.dnd_enabled());
}

#[test]
fn dnd_rollback_restores_state_when_write_is_still_current() {
    let mut config = Config::default();
    config.general.dnd_default = false;
    let mut store = NotificationStore::new(config);

    let write = store.set_dnd(true);
    assert!(store.dnd_enabled());

    // Simulate persistence failure with no newer writes in between.
    let rolled_back = store.rollback_dnd_write_if_current(&write);
    assert!(rolled_back);
    assert!(!store.dnd_enabled());
}

#[test]
fn next_id_skips_used_ids_within_used_window() {
    let mut store = make_store_with_limits(5, 5);
    store.next_id = 1;

    let mut active = make_notification("active");
    active.id = 1;
    store.active.insert(1, Arc::new(active));

    let mut history = make_notification("history");
    history.id = 3;
    store.history.insert(Arc::new(history));

    let id = store.next_id();
    assert_eq!(id, 2);
}

#[test]
fn inhibit_owner_mismatch_is_rejected() {
    let mut store = make_store_with_limits(10, 10);
    let id = store.add_inhibitor("owner-a".to_string(), "reason".to_string(), 0);
    let err = store
        .remove_inhibitor(id, "owner-b")
        .expect_err("owner mismatch should error");
    assert!(err.message().contains("owner-a"));
}

#[test]
fn is_notification_owned_by_matches_sender() {
    let mut store = make_store_with_limits(10, 10);
    let outcome = store.insert(
        make_notification_with_sender("owned", ":1.owner", 1234, 55),
        0,
    );
    assert!(store.is_notification_owned_by(
        outcome.notification.id,
        ":1.owner",
        Some(1234),
        Some(55)
    ));
    assert!(!store.is_notification_owned_by(
        outcome.notification.id,
        ":1.other",
        Some(5678),
        Some(66)
    ));
}

#[test]
fn is_notification_owned_by_accepts_same_process_after_reconnect() {
    let mut store = make_store_with_limits(10, 10);
    let outcome = store.insert(
        make_notification_with_sender("owned", ":1.owner-a", 1234, 55),
        0,
    );
    // A new bus name from the same process lifetime should still be treated as owner.
    assert!(store.is_notification_owned_by(
        outcome.notification.id,
        ":1.owner-b",
        Some(1234),
        Some(55)
    ));
}

#[test]
fn is_notification_owned_by_rejects_reused_pid_with_new_start_time() {
    let mut store = make_store_with_limits(10, 10);
    let outcome = store.insert(
        make_notification_with_sender("owned", ":1.owner-a", 1234, 55),
        0,
    );
    // Same pid is not enough once the original process lifetime has ended.
    assert!(!store.is_notification_owned_by(
        outcome.notification.id,
        ":1.owner-b",
        Some(1234),
        Some(77)
    ));
}

#[test]
fn inhibit_no_popups_suppresses_show_popup() {
    let mut config = Config::default();
    config.inhibit.mode = InhibitMode::NoPopups;
    let mut store = NotificationStore::new(config);
    store.add_inhibitor("owner".to_string(), "focus".to_string(), 0);

    let outcome = store.insert(make_notification("inhibited"), 0);
    assert!(!outcome.dropped);
    assert!(!outcome.show_popup);
    assert!(!outcome.allow_sound);
    assert_eq!(store.list_active().len(), 1);
}

#[test]
fn inhibit_drop_all_skips_storage() {
    let mut config = Config::default();
    config.inhibit.mode = InhibitMode::DropAll;
    let mut store = NotificationStore::new(config);
    store.add_inhibitor("owner".to_string(), "focus".to_string(), 0);

    let outcome = store.insert(make_notification("inhibited"), 0);
    assert!(outcome.dropped);
    assert!(store.list_active().is_empty());
    assert_eq!(store.history_len(), 0);
}
