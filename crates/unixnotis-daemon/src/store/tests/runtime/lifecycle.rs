use unixnotis_core::{CloseReason, NotificationKey};

use crate::store::test_support::{make_notification, make_store_with_limits};

#[test]
fn clear_all_removes_active_history_and_expiration_state_together() {
    let mut store = make_store_with_limits(10, 10);
    let active = store
        .insert(make_notification("active"), 0)
        .active_notification();
    let archived = store
        .insert(make_notification("archived"), 0)
        .active_notification();
    store.close(archived.id, CloseReason::Expired);
    store.set_expiration(&active, Some(std::time::Instant::now()));

    let removed = store.clear_all();

    assert_eq!(
        removed,
        vec![NotificationKey {
            id: active.id,
            generation: active.generation,
        }]
    );
    assert!(store.list_active().is_empty());
    assert!(store.list_history().is_empty());
    assert!(store.expirations.is_empty());
    assert!(store.popup_decisions.is_empty());
}
