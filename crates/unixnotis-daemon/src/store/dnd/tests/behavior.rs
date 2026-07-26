use super::*;

#[test]
fn plain_dnd_enable_replaces_a_timed_deadline_with_indefinite_state() {
    let state_dir = make_temp_state_dir("dnd-timed-to-indefinite");
    let mut store = NotificationStore::new_with_state_dir(Config::default(), state_dir.clone());
    let expires_at = chrono::Utc::now().timestamp() + 600;

    let timed = store.set_dnd_until(expires_at);
    assert!(timed.changed);
    assert_eq!(store.dnd_expires_at(), Some(expires_at));

    let indefinite = store.set_dnd(true);
    assert!(indefinite.changed);
    assert!(store.dnd_enabled());
    assert_eq!(store.dnd_expires_at(), None);
    cleanup_temp_dir(&state_dir);
}

#[test]
fn expiration_mutation_requires_the_current_due_deadline() {
    let state_dir = make_temp_state_dir("dnd-current-expiration");
    let mut store = NotificationStore::new_with_state_dir(Config::default(), state_dir.clone());
    let expires_at = 500;
    store.set_dnd_until(expires_at);

    assert!(
        !store
            .expire_dnd_if_current(expires_at + 1, expires_at)
            .changed
    );
    assert!(
        !store
            .expire_dnd_if_current(expires_at, expires_at - 1)
            .changed
    );
    assert!(store.dnd_enabled());

    let expired = store.expire_dnd_if_current(expires_at, expires_at);
    assert!(expired.changed);
    assert!(!store.dnd_enabled());
    assert_eq!(store.dnd_expires_at(), None);
    cleanup_temp_dir(&state_dir);
}

#[test]
fn dnd_toggle_flips_state_in_one_store_mutation() {
    let state_dir = make_temp_state_dir("dnd-toggle");
    let mut config = Config::default();
    config.general.dnd_default = false;
    let mut store = NotificationStore::new_with_state_dir(config, state_dir.clone());

    let first = store.toggle_dnd();
    assert!(first.changed);
    assert!(!first.previous);
    assert!(first.current);
    assert!(store.dnd_enabled());

    let second = store.toggle_dnd();
    assert!(second.changed);
    assert!(second.previous);
    assert!(!second.current);
    assert!(!store.dnd_enabled());

    cleanup_temp_dir(&state_dir);
}

#[test]
fn stale_dnd_rollback_cannot_overwrite_newer_write() {
    let state_dir = make_temp_state_dir("dnd-stale-rollback");
    let mut config = Config::default();
    config.general.dnd_default = false;
    let mut store = NotificationStore::new_with_state_dir(config, state_dir.clone());

    let write_a = store.set_dnd(true);
    assert!(store.dnd_enabled());

    let write_b = store.set_dnd(false);
    assert!(write_b.changed);
    assert!(!store.dnd_enabled());

    // Simulate late failure from write_a and verify guarded rollback is rejected
    let rolled_back = store.rollback_dnd_write_if_current(&write_a);
    assert!(!rolled_back);
    assert!(!store.dnd_enabled());

    cleanup_temp_dir(&state_dir);
}

#[test]
fn stale_dnd_rollback_cannot_overwrite_when_current_value_matches_old_write() {
    let state_dir = make_temp_state_dir("dnd-stale-current-matches");
    let mut config = Config::default();
    config.general.dnd_default = false;
    let mut store = NotificationStore::new_with_state_dir(config, state_dir.clone());

    let write_a = store.set_dnd(true);
    let _write_b = store.set_dnd(false);
    let _write_c = store.set_dnd(true);

    // Revision must win even when the current value happens to match the stale write
    assert!(!store.rollback_dnd_write_if_current(&write_a));
    assert!(store.dnd_enabled());

    cleanup_temp_dir(&state_dir);
}

#[test]
fn dnd_rollback_restores_state_when_write_is_still_current() {
    let state_dir = make_temp_state_dir("dnd-rollback");
    let mut config = Config::default();
    config.general.dnd_default = false;
    let mut store = NotificationStore::new_with_state_dir(config, state_dir.clone());

    let write = store.set_dnd(true);
    assert!(store.dnd_enabled());

    // Simulate persistence failure with no newer writes in between
    let rolled_back = store.rollback_dnd_write_if_current(&write);
    assert!(rolled_back);
    assert!(!store.dnd_enabled());

    cleanup_temp_dir(&state_dir);
}

#[test]
fn failed_timed_write_rollback_restores_the_previous_deadline() {
    let state_dir = make_temp_state_dir("dnd-timed-rollback");
    let mut store = NotificationStore::new_with_state_dir(Config::default(), state_dir.clone());
    let original = chrono::Utc::now().timestamp() + 600;
    let replacement = original + 600;
    store.set_dnd_until(original);

    let write = store.set_dnd_until(replacement);
    assert!(store.rollback_dnd_write_if_current(&write));

    assert!(store.dnd_enabled());
    assert_eq!(store.dnd_expires_at(), Some(original));
    cleanup_temp_dir(&state_dir);
}
