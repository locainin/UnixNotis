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
