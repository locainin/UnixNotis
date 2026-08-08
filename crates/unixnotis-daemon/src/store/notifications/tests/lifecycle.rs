use super::support::*;
use crate::store::ExpirationTicket;

fn expiration_for(store: &NotificationStore, id: u32) -> Option<ExpirationTicket> {
    // Test-only inspection stays beside lifecycle regressions instead of production methods
    store.expirations.get(&id).copied()
}

#[test]
fn drain_active_keys_returns_newest_first_and_clears_expirations() {
    let mut store = make_store_with_limits(10, 10);
    let first = store.insert(make_notification("first"), 0);
    let second = store.insert(make_notification("second"), 0);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    store.set_expiration(&first.active_notification(), Some(deadline));

    let keys = store.drain_active_keys();

    assert_eq!(
        keys,
        vec![
            second.active_notification().key(),
            first.active_notification().key()
        ]
    );
    assert!(store.list_active().is_empty());
    assert_eq!(expiration_for(&store, first.active_notification().id), None);
}

#[test]
fn expiration_bookkeeping_sets_replaces_and_removes_deadlines() {
    let mut store = make_store_with_limits(10, 10);
    let outcome = store.insert(make_notification("timer"), 0);
    let first = std::time::Instant::now() + std::time::Duration::from_secs(1);
    let second = std::time::Instant::now() + std::time::Duration::from_secs(2);

    let first_ticket = store
        .set_expiration(&outcome.active_notification(), Some(first))
        .expect("positive deadline should create a ticket");
    assert_eq!(
        expiration_for(&store, outcome.active_notification().id),
        Some(first_ticket)
    );

    let second_ticket = store
        .set_expiration(&outcome.active_notification(), Some(second))
        .expect("replacement deadline should create a ticket");
    assert_eq!(
        expiration_for(&store, outcome.active_notification().id),
        Some(second_ticket)
    );

    store.set_expiration(&outcome.active_notification(), None);
    assert_eq!(
        expiration_for(&store, outcome.active_notification().id),
        None
    );
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
    let original = store.insert(original, 0).active_notification();
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
    assert!(store.dismiss_active_if_current(id, &replacement.active_notification()));
    assert!(store.active_notification_view(id).is_none());
}

#[test]
fn stale_panel_dismissal_keeps_same_id_replacement() {
    let mut store = make_store_with_limits(12, 20);
    let original = store
        .insert(make_notification("original"), 0)
        .active_notification();
    let stale_key = original.key();
    let replacement = store
        .insert(make_notification("replacement"), original.id)
        .active_notification();

    let outcome = store.dismiss_generation(stale_key);

    assert!(!outcome.removed_any());
    assert_eq!(
        store
            .active_notification_view(replacement.id)
            .expect("replacement should remain active")
            .key(),
        replacement.key()
    );
}

#[test]
fn replied_generation_is_removed_after_sender_archives_it() {
    let mut store = make_store_with_limits(12, 20);
    let original = store
        .insert(make_notification("original"), 0)
        .active_notification();
    let id = original.id;
    store.close(id, CloseReason::ClosedByCall);
    assert_eq!(store.list_history().len(), 1);

    let outcome = store.dismiss_replied_generation(id, &original);

    assert!(outcome.removed_active.is_none());
    assert_eq!(outcome.removed_history, Some(original.key()));
    assert!(store.list_history().is_empty());
}

#[test]
fn replied_generation_cleanup_keeps_archived_same_id_replacement() {
    let mut store = make_store_with_limits(12, 20);
    let original = store
        .insert(make_notification("original"), 0)
        .active_notification();
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
