use super::support::*;

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
    let mut store = make_store_with_limits(32, 64);
    for index in 0..18 {
        store.insert(make_notification(&format!("entry-{index}")), 0);
    }

    let active = store.list_active();
    let history = store.list_history();

    assert_eq!(active.len(), 12);
    assert_eq!(history.len(), 6);
    assert_eq!(active[0].summary, "entry-17");
    assert_eq!(active[11].summary, "entry-6");
}

#[test]
fn zero_history_limit_keeps_active_notifications_and_drops_evictions() {
    let mut active_store = make_store_with_limits(2, 0);
    active_store.insert(make_notification("first"), 0);
    let active = active_store.insert(make_notification("second"), 0);
    assert!(active.evicted.is_empty());
    assert_eq!(active_store.list_active().len(), 2);

    let mut evicting_store = make_store_with_limits(0, 0);
    let evicted = evicting_store.insert(make_notification("first"), 0);
    assert_eq!(evicted.evicted.len(), 1);
    assert!(evicting_store.list_active().is_empty());
    assert_eq!(evicting_store.history_len(), 0);
}

#[test]
fn insert_outcome_reflects_popup_and_sound_policy() {
    let state_dir = make_temp_state_dir("insert-outcome-policy");
    let mut config = Config::default();
    config.general.dnd_default = false;
    let mut store = NotificationStore::new_with_state_dir(config, state_dir.clone());
    let allowed = store.insert(make_notification("normal"), 0);
    assert!(allowed.show_popup);
    assert!(allowed.allow_sound);

    let dnd_state_dir = make_temp_state_dir("insert-outcome-dnd");
    let mut dnd_config = Config::default();
    dnd_config.general.dnd_default = true;
    let mut dnd_store = NotificationStore::new_with_state_dir(dnd_config, dnd_state_dir.clone());
    let normal = dnd_store.insert(make_notification("normal dnd"), 0);
    assert!(!normal.show_popup);
    assert!(!normal.allow_sound);

    let mut critical = make_notification("critical dnd");
    critical.urgency = unixnotis_core::Urgency::Critical;
    let critical = dnd_store.insert(critical, 0);
    assert!(critical.show_popup);
    assert!(critical.allow_sound);

    let mut silent = make_notification("silent");
    silent.suppress_sound = true;
    let silent = store.insert(silent, 0);
    assert!(!silent.allow_sound);

    cleanup_temp_dir(&state_dir);
    cleanup_temp_dir(&dnd_state_dir);
}
