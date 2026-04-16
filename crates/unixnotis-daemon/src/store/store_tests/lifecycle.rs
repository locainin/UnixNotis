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
