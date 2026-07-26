use super::support::*;

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
    let state_store = super::super::persistence::DndStateStore::from_state_dir(state_dir.clone());

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
    let state_store = super::super::persistence::DndStateStore::from_state_dir(state_dir.clone());

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
    let state_store = super::super::persistence::DndStateStore::from_state_dir(state_dir.clone());

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
