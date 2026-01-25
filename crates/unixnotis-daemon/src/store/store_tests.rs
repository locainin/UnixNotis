//! Store regression coverage and persistence validation.

use super::store_state::{PersistedDndState, DND_STATE_FILE, DND_STATE_VERSION};
use super::{contains_ci, NotificationStore};
use chrono::Utc;
use std::collections::HashMap;
use unixnotis_core::{Config, InhibitMode, Notification, NotificationImage, Urgency};
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
    }
}

fn make_store_with_limits(max_active: usize, max_entries: usize) -> NotificationStore {
    let mut config = Config::default();
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
    std::fs::create_dir_all(path.parent().expect("state parent"))
        .expect("create state directory");
    std::fs::write(&path, payload).expect("write state");
}

fn cleanup_temp_dir(dir: &std::path::Path) {
    let _ = std::fs::remove_dir_all(dir);
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
fn max_entries_zero_drops_history_on_close() {
    let mut store = make_store_with_limits(10, 0);

    let outcome = store.insert(make_notification("first"), 0);
    store.close(outcome.notification.id);

    assert_eq!(store.history_len(), 0);
}

#[test]
fn replace_id_in_history_reuses_id_and_clears_entry() {
    let mut store = make_store_with_limits(2, 10);

    let first = store.insert(make_notification("first"), 0);
    store.close(first.notification.id);
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
    store.close(replaced.notification.id);
    let history = store.list_history();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].summary, "replacement");
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
    std::fs::create_dir_all(path.parent().expect("state parent"))
        .expect("create state directory");
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
    assert!(store.set_dnd(true));

    let path = state_dir.join("unixnotis").join(DND_STATE_FILE);
    let contents = std::fs::read_to_string(&path).expect("read persisted state");
    let parsed: PersistedDndState =
        serde_json::from_str(&contents).expect("parse persisted state");
    assert!(parsed.dnd_enabled);

    cleanup_temp_dir(&state_dir);
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
fn inhibit_no_popups_suppresses_show_popup() {
    let mut config = Config::default();
    config.inhibit.mode = InhibitMode::NoPopups;
    let mut store = NotificationStore::new(config);
    store.add_inhibitor("owner".to_string(), "focus".to_string(), 0);

    let outcome = store.insert(make_notification("inhibited"), 0);
    assert!(!outcome.dropped);
    assert!(!outcome.show_popup);
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
