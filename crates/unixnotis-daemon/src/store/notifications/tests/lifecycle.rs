use super::support::*;

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
fn generation_safe_reply_dismissal_keeps_same_id_replacement() {
    let mut store = make_store_with_limits(12, 20);
    let mut original = make_notification("original");
    original.inline_reply.available = true;
    original.actions.push(unixnotis_core::Action {
        key: "inline-reply".to_string(),
        label: "Reply".to_string(),
    });
    let original = store.insert(original, 0).notification;
    let id = original.id;

    let replacement = store.insert(make_notification("replacement"), id);
    assert!(replacement.replaced);

    assert!(!store.dismiss_active_if_current(id, &original));
    assert_eq!(
        store
            .active_notification_view(id)
            .expect("replacement should remain active")
            .summary,
        "replacement"
    );
    assert!(store.dismiss_active_if_current(id, &replacement.notification));
    assert!(store.active_notification_view(id).is_none());
}

#[test]
fn replied_generation_is_removed_after_sender_archives_it() {
    let mut store = make_store_with_limits(12, 20);
    let original = store.insert(make_notification("original"), 0).notification;
    let id = original.id;
    store.close(id, CloseReason::ClosedByCall);
    assert_eq!(store.list_history().len(), 1);

    let outcome = store.dismiss_replied_generation(id, &original);

    assert!(!outcome.removed_active);
    assert!(outcome.removed_history);
    assert!(store.list_history().is_empty());
}

#[test]
fn replied_generation_cleanup_keeps_archived_same_id_replacement() {
    let mut store = make_store_with_limits(12, 20);
    let original = store.insert(make_notification("original"), 0).notification;
    let id = original.id;
    let replacement = store.insert(make_notification("replacement"), id);
    assert!(replacement.replaced);
    store.close(id, CloseReason::ClosedByCall);

    let outcome = store.dismiss_replied_generation(id, &original);

    assert!(!outcome.removed_any());
    let history = store.list_history();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].summary, "replacement");
}
