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
fn dnd_toggle_flips_state_in_one_store_mutation() {
    let mut config = Config::default();
    config.general.dnd_default = false;
    let mut store = NotificationStore::new(config);

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
}

#[test]
fn stale_dnd_rollback_cannot_overwrite_newer_write() {
    let mut config = Config::default();
    config.general.dnd_default = false;
    let mut store = NotificationStore::new(config);

    let write_a = store.set_dnd(true);
    assert!(store.dnd_enabled());

    let write_b = store.set_dnd(false);
    assert!(write_b.changed);
    assert!(!store.dnd_enabled());

    // Simulate late failure from write_a and verify guarded rollback is rejected.
    let rolled_back = store.rollback_dnd_write_if_current(&write_a);
    assert!(!rolled_back);
    assert!(!store.dnd_enabled());
}

#[test]
fn dnd_rollback_restores_state_when_write_is_still_current() {
    let mut config = Config::default();
    config.general.dnd_default = false;
    let mut store = NotificationStore::new(config);

    let write = store.set_dnd(true);
    assert!(store.dnd_enabled());

    // Simulate persistence failure with no newer writes in between.
    let rolled_back = store.rollback_dnd_write_if_current(&write);
    assert!(rolled_back);
    assert!(!store.dnd_enabled());
}
