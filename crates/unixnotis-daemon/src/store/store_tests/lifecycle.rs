use super::*;

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
fn config_accessor_returns_runtime_config_snapshot() {
    let mut config = Config::default();
    config.history.max_entries = 77;
    config.history.max_active = 3;
    let store = NotificationStore::new(config);

    assert_eq!(store.config().history.max_entries, 77);
    assert_eq!(store.config().history.max_active, 3);
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
    // Config may request a larger active window, but runtime hard-cap protects UI stability
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
fn max_entries_zero_keeps_active_notifications_when_active_limit_allows() {
    let mut store = make_store_with_limits(2, 0);

    store.insert(make_notification("first"), 0);
    let outcome = store.insert(make_notification("second"), 0);

    assert!(outcome.evicted.is_empty());
    assert_eq!(store.list_active().len(), 2);
    assert_eq!(store.history_len(), 0);
}

#[test]
fn history_eviction_keeps_most_recent_entries() {
    let mut store = make_store_with_limits(0, 2);

    store.insert(make_notification("first"), 0);
    store.insert(make_notification("second"), 0);
    store.insert(make_notification("third"), 0);

    // History listing returns most-recent-first order
    let history = store.list_history();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].summary, "third");
    assert_eq!(history[1].summary, "second");
}

#[test]
fn history_reinsert_replaces_existing_order_entry() {
    let mut store = make_store_with_limits(0, 10);
    let first = store.insert(make_notification("first"), 0);
    assert_eq!(store.history_len(), 1);

    let mut replacement = make_notification("replacement");
    replacement.id = first.notification.id;
    store.history.insert(Arc::new(replacement));

    // Replacing an archived id must not leave a stale duplicate in history order
    let history = store.list_history();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].id, first.notification.id);
    assert_eq!(history[0].summary, "replacement");
}

#[test]
fn max_entries_zero_drops_history_on_insert() {
    let mut store = make_store_with_limits(0, 0);

    let outcome = store.insert(make_notification("first"), 0);

    // Eviction should archive the active entry, then drop it due to the zero history limit
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
fn next_id_skips_ids_that_exist_only_in_history() {
    let mut store = make_store_with_limits(5, 5);
    store.next_id = 7;

    let mut history = make_notification("history-only");
    history.id = 7;
    store.history.insert(Arc::new(history));

    // History IDs still belong to notification identity and must not be reused
    assert_eq!(store.next_id(), 8);
}

#[test]
fn next_id_wraps_internal_cursor_back_to_one_after_max_id() {
    let mut store = make_store_with_limits(5, 5);
    store.next_id = u32::MAX;

    assert_eq!(store.next_id(), u32::MAX);
    // The stored cursor must not remain zero after wrapping past u32::MAX
    assert_eq!(store.next_id, 1);
}

#[test]
fn clear_history_removes_archived_notifications() {
    let mut store = make_store_with_limits(10, 10);
    let first = store.insert(make_notification("first"), 0);
    store.close(first.notification.id, CloseReason::Expired);

    assert_eq!(store.history_len(), 1);
    store.clear_history();
    assert_eq!(store.history_len(), 0);
    assert!(store.list_history().is_empty());
}

#[test]
fn dismiss_outcome_reports_any_removed_side() {
    assert!(crate::store::DismissOutcome {
        removed_active: true,
        removed_history: false,
    }
    .removed_any());
    assert!(crate::store::DismissOutcome {
        removed_active: false,
        removed_history: true,
    }
    .removed_any());
    assert!(!crate::store::DismissOutcome {
        removed_active: false,
        removed_history: false,
    }
    .removed_any());
}

#[test]
fn active_notification_view_returns_current_active_payload() {
    let mut store = make_store_with_limits(10, 10);
    let outcome = store.insert(make_notification("visible"), 0);

    let view = store
        .active_notification_view(outcome.notification.id)
        .expect("active notification should be visible");

    assert_eq!(view.id, outcome.notification.id);
    assert_eq!(view.summary, "visible");
}

#[test]
fn drain_active_ids_returns_newest_first_and_clears_expirations() {
    let mut store = make_store_with_limits(10, 10);
    let first = store.insert(make_notification("first"), 0);
    let second = store.insert(make_notification("second"), 0);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    store.set_expiration(first.notification.id, Some(deadline));

    let ids = store.drain_active_ids();

    assert_eq!(ids, vec![second.notification.id, first.notification.id]);
    assert!(store.list_active().is_empty());
    assert_eq!(store.expiration_for(first.notification.id), None);
}

#[test]
fn expiration_bookkeeping_sets_replaces_and_removes_deadlines() {
    let mut store = make_store_with_limits(10, 10);
    let outcome = store.insert(make_notification("timer"), 0);
    let first = std::time::Instant::now() + std::time::Duration::from_secs(1);
    let second = std::time::Instant::now() + std::time::Duration::from_secs(2);

    store.set_expiration(outcome.notification.id, Some(first));
    assert_eq!(store.expiration_for(outcome.notification.id), Some(first));

    store.set_expiration(outcome.notification.id, Some(second));
    assert_eq!(store.expiration_for(outcome.notification.id), Some(second));

    store.set_expiration(outcome.notification.id, None);
    assert_eq!(store.expiration_for(outcome.notification.id), None);
}

#[test]
fn insert_outcome_reflects_popup_and_sound_policy() {
    let state_dir = make_temp_state_dir("insert-outcome-policy");
    let mut config = Config::default();
    config.general.dnd_default = false;
    let mut store = NotificationStore::new_with_state_dir(config, state_dir.clone());
    let allowed = store.insert(make_notification("normal"), 0);
    assert!(allowed.show_popup);
    assert!(allowed.allow_sound);

    let dnd_state_dir = make_temp_state_dir("insert-outcome-dnd");
    let mut dnd_config = Config::default();
    dnd_config.general.dnd_default = true;
    let mut dnd_store = NotificationStore::new_with_state_dir(dnd_config, dnd_state_dir.clone());
    let normal = dnd_store.insert(make_notification("normal dnd"), 0);
    assert!(!normal.show_popup);
    assert!(!normal.allow_sound);

    let mut critical = make_notification("critical dnd");
    critical.urgency = unixnotis_core::Urgency::Critical;
    let critical = dnd_store.insert(critical, 0);
    assert!(critical.show_popup);
    assert!(critical.allow_sound);

    let mut silent = make_notification("silent");
    silent.suppress_sound = true;
    let silent = store.insert(silent, 0);
    assert!(!silent.allow_sound);

    cleanup_temp_dir(&state_dir);
    cleanup_temp_dir(&dnd_state_dir);
}
