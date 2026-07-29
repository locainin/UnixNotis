use super::support::*;

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

    let history = store.list_history();

    assert_eq!(history.len(), 2);
    assert_eq!(history[0].summary, "third");
    assert_eq!(history[1].summary, "second");
}

#[test]
fn history_reinsert_replaces_existing_order_entry() {
    let mut store = make_store_with_limits(0, 10);
    let first = store.insert(make_notification("first"), 0);
    let mut replacement = make_notification("replacement");
    replacement.id = first.notification.id;

    store.history.insert(Arc::new(replacement));

    let history = store.list_history();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].id, first.notification.id);
    assert_eq!(history[0].summary, "replacement");
}

#[test]
fn transient_close_obeys_the_history_policy() {
    for (enabled, expected) in [(false, 0), (true, 1)] {
        let mut config = Config::default();
        config.history.transient_to_history = enabled;
        let mut store = NotificationStore::new(config);
        let mut notification = make_notification("transient");
        notification.is_transient = true;
        let outcome = store.insert(notification, 0);

        store.close(outcome.notification.id, CloseReason::Expired);

        assert_eq!(store.history_len(), expected);
    }
}

#[test]
fn clear_history_removes_archived_notifications() {
    let mut store = make_store_with_limits(10, 10);
    let first = store.insert(make_notification("first"), 0);
    store.close(first.notification.id, CloseReason::Expired);

    store.clear_history();

    assert_eq!(store.history_len(), 0);
    assert!(store.list_history().is_empty());
}

#[test]
fn history_generation_checks_and_removal_require_the_exact_commit_key() {
    let mut store = make_store_with_limits(10, 10);
    let notification = store.insert(make_notification("archived"), 0).notification;
    let current = notification.key();
    let stale = unixnotis_core::NotificationKey {
        id: current.id,
        generation: current.generation.saturating_add(1),
    };
    store.close(current.id, CloseReason::Expired);

    assert!(store.history.contains_generation(current));
    assert!(!store.history.contains_generation(stale));
    assert!(store.history.remove_generation(stale).is_none());
    assert!(store.history.contains_generation(current));

    let removed = store
        .history
        .remove_generation(current)
        .expect("current generation should be removable");
    assert_eq!(removed.key(), current);
    assert!(!store.history.contains_generation(current));
}
