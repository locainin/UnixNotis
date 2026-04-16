use super::*;

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
