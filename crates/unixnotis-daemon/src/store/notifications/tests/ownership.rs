use super::support::*;

fn is_notification_owned_by(
    store: &NotificationStore,
    id: u32,
    sender: &str,
    sender_pid: Option<u32>,
    sender_start_time: Option<u64>,
) -> bool {
    matches!(
        store.close_authorization(id, Some(sender), sender_pid, sender_start_time),
        crate::store::CloseAuthorization::OwnedActive(_)
    )
}

#[test]
fn replace_id_in_history_allocates_new_id_and_preserves_history() {
    let mut store = make_store_with_limits(2, 10);

    let first = store.insert(make_notification("first"), 0);
    store.close(first.active_notification().id, CloseReason::Expired);
    assert_eq!(store.history_len(), 1);

    // History cannot restore replacement authority for an inactive protocol ID
    let replaced = store.insert(
        make_notification("replacement"),
        first.active_notification().id,
    );
    assert!(!replaced.replaced);
    assert_ne!(
        replaced.active_notification().id,
        first.active_notification().id
    );
    assert_eq!(store.history_len(), 1);

    let active = store.list_active();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].summary, "replacement");

    // Closing the new notification archives it independently from the original ID
    store.close(replaced.active_notification().id, CloseReason::Expired);
    let history = store.list_history();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].summary, "replacement");
    assert_eq!(history[1].summary, "first");
}

#[test]
fn active_owned_id_replaces_while_active_foreign_and_missing_ids_do_not() {
    let mut store = make_store_with_limits(5, 10);
    let owned = store.insert(
        make_notification_with_sender("owned", ":1.owner", 101, 11),
        0,
    );
    let foreign = store.insert(
        make_notification_with_sender("foreign", ":1.foreign", 202, 22),
        0,
    );

    let owned_replacement = store.insert(
        make_notification_with_sender("owned replacement", ":1.owner", 101, 11),
        owned.active_notification().id,
    );
    let foreign_attempt = store.insert(
        make_notification_with_sender("foreign attempt", ":1.owner", 101, 11),
        foreign.active_notification().id,
    );
    let missing_attempt = store.insert(
        make_notification_with_sender("missing attempt", ":1.owner", 101, 11),
        u32::MAX,
    );

    assert!(owned_replacement.replaced);
    assert_eq!(
        owned_replacement.active_notification().id,
        owned.active_notification().id
    );
    assert!(!foreign_attempt.replaced);
    assert_ne!(
        foreign_attempt.active_notification().id,
        foreign.active_notification().id
    );
    assert!(!missing_attempt.replaced);
    assert_ne!(missing_attempt.active_notification().id, u32::MAX);
}

#[test]
fn replace_id_rejected_for_different_sender() {
    let mut store = make_store_with_limits(2, 10);

    let first = store.insert(
        make_notification_with_sender("first", ":1.sender-a", 101, 1),
        0,
    );
    store.close(first.active_notification().id, CloseReason::Expired);
    assert_eq!(store.history_len(), 1);

    // Cross-sender replacement must allocate a fresh id and keep prior history intact
    let replaced = store.insert(
        make_notification_with_sender("replacement", ":1.sender-b", 202, 2),
        first.active_notification().id,
    );
    assert!(!replaced.replaced);
    assert_ne!(
        replaced.active_notification().id,
        first.active_notification().id
    );
    assert_eq!(store.history_len(), 1);
}

#[test]
fn is_notification_owned_by_matches_sender() {
    let mut store = make_store_with_limits(10, 10);
    let outcome = store.insert(
        make_notification_with_sender("owned", ":1.owner", 1234, 55),
        0,
    );
    assert!(is_notification_owned_by(
        &store,
        outcome.active_notification().id,
        ":1.owner",
        Some(1234),
        Some(55)
    ));
    assert!(!is_notification_owned_by(
        &store,
        outcome.active_notification().id,
        ":1.other",
        Some(5678),
        Some(66)
    ));
}

#[test]
fn is_notification_owned_by_accepts_exact_sender_without_process_match() {
    let mut store = make_store_with_limits(10, 10);
    let outcome = store.insert(
        make_notification_with_sender("owned", ":1.owner", 1234, 55),
        0,
    );

    // Bus names are stronger than pid metadata, which may be absent or stale
    assert!(is_notification_owned_by(
        &store,
        outcome.active_notification().id,
        ":1.owner",
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
    // A new bus name from the same process lifetime should still be treated as owner
    assert!(is_notification_owned_by(
        &store,
        outcome.active_notification().id,
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
    // Same pid is not enough once the original process lifetime has ended
    assert!(!is_notification_owned_by(
        &store,
        outcome.active_notification().id,
        ":1.owner-b",
        Some(1234),
        Some(77)
    ));
}

#[test]
fn is_notification_owned_by_rejects_pid_match_without_start_time() {
    let mut store = make_store_with_limits(10, 10);
    let outcome = store.insert(
        make_notification_with_sender("owned", ":1.owner-a", 1234, 55),
        0,
    );

    // Pid reuse is common enough that start time must be part of process ownership
    assert!(!is_notification_owned_by(
        &store,
        outcome.active_notification().id,
        ":1.owner-b",
        Some(1234),
        None
    ));
}

#[test]
fn close_authorization_collapses_missing_foreign_and_history_only_ids() {
    let mut store = make_store_with_limits(10, 10);
    let active = store
        .insert(
            make_notification_with_sender("active", ":1.owner", 1234, 55),
            0,
        )
        .active_notification();
    let archived = store
        .insert(
            make_notification_with_sender("archived", ":1.owner", 1234, 55),
            0,
        )
        .active_notification();
    store.close(archived.id, CloseReason::Expired);

    let missing = store.close_authorization(u32::MAX, Some(":1.owner"), Some(1234), Some(55));
    let foreign = store.close_authorization(active.id, Some(":1.foreign"), Some(9876), Some(66));
    let history = store.close_authorization(archived.id, Some(":1.owner"), Some(1234), Some(55));

    assert_eq!(missing, crate::store::CloseAuthorization::NotClosable);
    assert_eq!(foreign, crate::store::CloseAuthorization::NotClosable);
    assert_eq!(history, crate::store::CloseAuthorization::NotClosable);
    assert_eq!(missing, foreign);
    assert_eq!(foreign, history);
}

#[test]
fn close_owned_active_generation_removes_only_the_authorized_live_object() {
    let mut store = make_store_with_limits(10, 10);
    let active = store
        .insert(
            make_notification_with_sender("active", ":1.owner-a", 1234, 55),
            0,
        )
        .active_notification();

    let removed = store
        .close_owned_active_generation(
            active.key(),
            Some(":1.owner-b"),
            Some(1234),
            Some(55),
            CloseReason::ClosedByCall,
        )
        .expect("same process lifetime should close after reconnect");

    assert_eq!(removed.key(), active.key());
    assert!(store.list_active().is_empty());
}

#[test]
fn close_owned_active_generation_rejects_a_same_id_replacement() {
    let mut store = make_store_with_limits(10, 10);
    let original = store
        .insert(
            make_notification_with_sender("original", ":1.owner", 1234, 55),
            0,
        )
        .active_notification();
    let replacement = store
        .insert(
            make_notification_with_sender("replacement", ":1.owner", 1234, 55),
            original.id,
        )
        .active_notification();

    let removed = store.close_owned_active_generation(
        original.key(),
        Some(":1.owner"),
        Some(1234),
        Some(55),
        CloseReason::ClosedByCall,
    );

    assert!(removed.is_none());
    assert_eq!(
        store.active.get(&replacement.id).map(|item| item.key()),
        Some(replacement.key())
    );
}

#[test]
fn replacement_allows_same_process_after_bus_reconnect() {
    let mut store = make_store_with_limits(2, 10);

    let first = store.insert(
        make_notification_with_sender("first", ":1.owner-a", 1234, 55),
        0,
    );

    let replacement = store.insert(
        make_notification_with_sender("replacement", ":1.owner-b", 1234, 55),
        first.active_notification().id,
    );

    // Same process lifetime can replace after the bus name changes
    assert!(replacement.replaced);
    assert_eq!(
        replacement.active_notification().id,
        first.active_notification().id
    );
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

    assert_eq!(store.next_id(), 2);
}

#[test]
fn next_id_skips_ids_that_exist_only_in_history() {
    let mut store = make_store_with_limits(5, 5);
    store.next_id = 7;

    let mut history = make_notification("history-only");
    history.id = 7;
    store.history.insert(Arc::new(history));

    assert_eq!(store.next_id(), 8);
}

#[test]
fn next_id_wraps_internal_cursor_back_to_one_after_max_id() {
    let mut store = make_store_with_limits(5, 5);
    store.next_id = u32::MAX;

    assert_eq!(store.next_id(), u32::MAX);
    assert_eq!(store.next_id, 1);
}
