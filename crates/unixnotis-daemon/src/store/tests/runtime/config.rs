use unixnotis_core::Config;

use crate::store::test_support::{make_notification, make_store_with_limits};
use crate::store::NotificationStore;

#[test]
fn config_accessor_returns_runtime_config_snapshot() {
    let mut config = Config::default();
    config.history.max_entries = 77;
    config.history.max_active = 3;
    let store = NotificationStore::new(config);

    assert_eq!(store.config.history.max_entries, 77);
    assert_eq!(store.config.history.max_active, 3);
}

#[test]
fn active_notification_view_returns_current_active_payload() {
    let mut store = make_store_with_limits(10, 10);
    let outcome = store.insert(make_notification("visible"), 0);

    let view = store
        .active_notification_view(outcome.active_notification().id)
        .expect("active notification should be visible");

    assert_eq!(view.id, outcome.active_notification().id);
    assert_eq!(view.summary, "visible");
}
