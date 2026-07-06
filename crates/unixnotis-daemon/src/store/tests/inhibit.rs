use super::*;

#[test]
fn inhibit_no_popups_suppresses_show_popup() {
    let mut config = Config::default();
    config.inhibit.mode = InhibitMode::NoPopups;
    let mut store = NotificationStore::new(config);
    store.add_inhibitor("owner".to_string(), "focus".to_string(), 0);

    let outcome = store.insert(make_notification("inhibited"), 0);
    assert!(!outcome.dropped);
    assert!(!outcome.show_popup);
    assert!(!outcome.allow_sound);
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

#[test]
fn inhibit_scope_zero_and_popup_bit_mark_store_inhibited() {
    let mut store = NotificationStore::new(Config::default());

    let all = store.add_inhibitor("owner-a".to_string(), "all".to_string(), 0);
    assert!(store.inhibited());
    assert_eq!(store.inhibitor_count(), 1);

    assert!(store
        .remove_inhibitor(all, "owner-a")
        .expect("owner can remove inhibitor"));
    assert!(!store.inhibited());
    assert_eq!(store.inhibitor_count(), 0);

    store.add_inhibitor(
        "owner-b".to_string(),
        "popups".to_string(),
        unixnotis_core::INHIBIT_SCOPE_POPUPS,
    );
    assert!(store.inhibited());
}

#[test]
fn unrelated_inhibit_scope_does_not_suppress_popups() {
    let mut store = NotificationStore::new(Config::default());

    store.add_inhibitor("owner".to_string(), "other".to_string(), 0b10);

    assert!(!store.inhibited());
    assert_eq!(store.inhibitor_count(), 1);
}

#[test]
fn remove_inhibitors_by_owner_only_removes_matching_owner() {
    let mut store = NotificationStore::new(Config::default());
    let first = store.add_inhibitor("owner-a".to_string(), "first".to_string(), 0);
    let second = store.add_inhibitor("owner-b".to_string(), "second".to_string(), 0);

    assert!(store.remove_inhibitors_by_owner("owner-a"));
    assert!(!store.remove_inhibitors_by_owner("owner-a"));

    let inhibitors = store.list_inhibitors();
    assert_eq!(
        inhibitors,
        vec![(second, "second".to_string(), 0, "owner-b".to_string())]
    );
    assert!(store
        .remove_inhibitor(second, "owner-b")
        .expect("remove owner-b"));
    assert!(!store
        .remove_inhibitor(first, "owner-a")
        .expect("missing is idempotent"));
}

#[test]
fn list_inhibitors_is_sorted_and_preserves_fields() {
    let mut store = NotificationStore::new(Config::default());
    let first = store.add_inhibitor("owner-a".to_string(), "reason-a".to_string(), 0);
    let second = store.add_inhibitor(
        "owner-b".to_string(),
        "reason-b".to_string(),
        unixnotis_core::INHIBIT_SCOPE_POPUPS,
    );

    assert_eq!(
        store.list_inhibitors(),
        vec![
            (first, "reason-a".to_string(), 0, "owner-a".to_string()),
            (
                second,
                "reason-b".to_string(),
                unixnotis_core::INHIBIT_SCOPE_POPUPS,
                "owner-b".to_string()
            ),
        ]
    );
}
