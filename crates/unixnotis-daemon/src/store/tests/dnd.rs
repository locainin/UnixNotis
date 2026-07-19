use super::*;

#[test]
fn dnd_state_overrides_default() {
    let state_dir = make_temp_state_dir("dnd-override");
    write_dnd_state(&state_dir, true, DND_STATE_VERSION);

    let mut config = Config::default();
    config.general.dnd_default = false;
    let store = NotificationStore::new_with_state_dir(config, state_dir.clone());
    assert!(store.dnd_enabled());

    cleanup_temp_dir(&state_dir);
}

#[test]
fn dnd_state_invalid_payload_falls_back_to_default() {
    let state_dir = make_temp_state_dir("dnd-invalid");
    let path = state_dir.join("unixnotis").join(DND_STATE_FILE);
    std::fs::create_dir_all(path.parent().expect("state parent")).expect("create state directory");
    std::fs::write(&path, "{").expect("write invalid state");

    let mut config = Config::default();
    config.general.dnd_default = true;
    let store = NotificationStore::new_with_state_dir(config, state_dir.clone());
    assert!(store.dnd_enabled());

    cleanup_temp_dir(&state_dir);
}

#[test]
fn dnd_state_store_load_returns_none_when_file_is_missing() {
    let state_dir = make_temp_state_dir("dnd-missing-file");
    let state_store = super::super::state::DndStateStore::from_state_dir(state_dir.clone());

    // A first run has no state file yet, which should not be treated as corruption
    let loaded = state_store
        .load()
        .expect("missing state file should be valid");
    assert!(loaded.is_none());

    cleanup_temp_dir(&state_dir);
}

#[test]
fn dnd_state_store_load_reports_non_missing_filesystem_errors() {
    let state_dir = make_temp_state_dir("dnd-path-is-directory");
    let path = state_dir.join("unixnotis").join(DND_STATE_FILE);
    std::fs::create_dir_all(&path).expect("create directory at state file path");
    let state_store = super::super::state::DndStateStore::from_state_dir(state_dir.clone());

    // Wrong path shape is a real filesystem problem and should not look like first run
    let err = state_store
        .load()
        .expect_err("directory state path should fail");
    assert_ne!(err.kind(), std::io::ErrorKind::NotFound);

    cleanup_temp_dir(&state_dir);
}

#[test]
fn dnd_state_unsupported_version_falls_back_to_default() {
    let state_dir = make_temp_state_dir("dnd-unsupported-version");
    write_dnd_state(&state_dir, true, DND_STATE_VERSION + 1);

    let mut config = Config::default();
    config.general.dnd_default = false;
    let store = NotificationStore::new_with_state_dir(config, state_dir.clone());
    assert!(!store.dnd_enabled());

    cleanup_temp_dir(&state_dir);
}

#[test]
fn dnd_state_persists_on_change() {
    let state_dir = make_temp_state_dir("dnd-write");
    let mut config = Config::default();
    config.general.dnd_default = false;
    let mut store = NotificationStore::new_with_state_dir(config, state_dir.clone());
    assert!(apply_dnd_update(&mut store, true));

    let path = state_dir.join("unixnotis").join(DND_STATE_FILE);
    let contents = std::fs::read_to_string(&path).expect("read persisted state");
    let parsed: PersistedDndState = serde_json::from_str(&contents).expect("parse persisted state");
    assert!(parsed.dnd_enabled);

    cleanup_temp_dir(&state_dir);
}

#[test]
fn timed_dnd_persists_the_absolute_deadline() {
    let state_dir = make_temp_state_dir("dnd-timed-write");
    let mut store = NotificationStore::new_with_state_dir(Config::default(), state_dir.clone());
    let expires_at = chrono::Utc::now().timestamp() + 3_600;
    let write = store.set_dnd_until(expires_at);
    write
        .persist
        .as_ref()
        .expect("timed DND state store")
        .persist(write.current, write.current_expires_at)
        .expect("persist timed DND");

    let path = state_dir.join("unixnotis").join(DND_STATE_FILE);
    let persisted: PersistedDndState =
        serde_json::from_slice(&std::fs::read(path).expect("read timed DND state"))
            .expect("parse timed DND state");

    assert!(persisted.dnd_enabled);
    assert_eq!(persisted.expires_at, Some(expires_at));
    cleanup_temp_dir(&state_dir);
}

#[cfg(unix)]
#[test]
fn dnd_state_persistence_rejects_symlink_without_touching_outside_file() {
    use std::os::unix::fs::symlink;

    let state_dir = make_temp_state_dir("dnd-write-symlink");
    let state_parent = state_dir.join("unixnotis");
    std::fs::create_dir_all(&state_parent).expect("create state directory");
    let outside = state_dir.join("outside.json");
    std::fs::write(&outside, "keep").expect("write outside state");
    symlink(&outside, state_parent.join(DND_STATE_FILE)).expect("create state symlink");
    let state_store = super::super::state::DndStateStore::from_state_dir(state_dir.clone());

    let error = state_store
        .persist(true, None)
        .expect_err("state symlink should be rejected");

    assert_ne!(error.kind(), std::io::ErrorKind::NotFound);
    assert_eq!(
        std::fs::read_to_string(outside).expect("read outside state"),
        "keep"
    );
    cleanup_temp_dir(&state_dir);
}

#[test]
fn future_timed_dnd_is_loaded_with_its_deadline() {
    let state_dir = make_temp_state_dir("dnd-future-deadline");
    let expires_at = chrono::Utc::now().timestamp() + 3_600;
    let state = PersistedDndState {
        version: DND_STATE_VERSION,
        dnd_enabled: true,
        expires_at: Some(expires_at),
        updated_at: None,
    };
    let path = state_dir.join("unixnotis").join(DND_STATE_FILE);
    std::fs::create_dir_all(path.parent().expect("state parent")).expect("create state directory");
    std::fs::write(
        &path,
        serde_json::to_vec(&state).expect("serialize timed state"),
    )
    .expect("write timed state");

    let store = NotificationStore::new_with_state_dir(Config::default(), state_dir.clone());

    assert!(store.dnd_enabled());
    assert_eq!(store.dnd_expires_at(), Some(expires_at));
    cleanup_temp_dir(&state_dir);
}

#[test]
fn expired_timed_dnd_is_disabled_during_startup_and_cleared_on_disk() {
    let state_dir = make_temp_state_dir("dnd-expired-deadline");
    let state = PersistedDndState {
        version: DND_STATE_VERSION,
        dnd_enabled: true,
        expires_at: Some(chrono::Utc::now().timestamp() - 1),
        updated_at: None,
    };
    let path = state_dir.join("unixnotis").join(DND_STATE_FILE);
    std::fs::create_dir_all(path.parent().expect("state parent")).expect("create state directory");
    std::fs::write(
        &path,
        serde_json::to_vec(&state).expect("serialize expired state"),
    )
    .expect("write expired state");

    let store = NotificationStore::new_with_state_dir(Config::default(), state_dir.clone());
    let persisted: PersistedDndState =
        serde_json::from_slice(&std::fs::read(&path).expect("read corrected persisted state"))
            .expect("parse corrected state");

    assert!(!store.dnd_enabled());
    assert_eq!(store.dnd_expires_at(), None);
    assert!(!persisted.dnd_enabled);
    assert_eq!(persisted.expires_at, None);
    cleanup_temp_dir(&state_dir);
}

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
