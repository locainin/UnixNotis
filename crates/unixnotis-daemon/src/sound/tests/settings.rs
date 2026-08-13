use super::*;
use crate::sound::SoundFile;
use crate::system_tools::routing::use_fake_tool_bin;
use crate::test_support::TempRoot;
use zbus::zvariant::{OwnedValue, Value};

fn settings(enabled: bool, backend: SoundBackend) -> SoundSettings {
    SoundSettings {
        enabled,
        backend,
        allow_file_hints: false,
        allowed_file_hint_dirs: Vec::new(),
        default_name: Some("message-new-instant".to_string()),
        default_file: None,
        last_played: Mutex::new(None),
    }
}

fn suppress_sound_hints(value: bool) -> HashMap<String, OwnedValue> {
    let mut hints = HashMap::new();
    hints.insert(
        "suppress-sound".to_string(),
        Value::from(value)
            .try_into()
            .expect("bool hint should convert"),
    );
    hints
}

fn last_played_is_set(settings: &SoundSettings) -> bool {
    settings
        .last_played
        .lock()
        .expect("last_played lock")
        .is_some()
}

fn install_fake_canberra(root: &TempRoot) {
    use std::os::unix::fs::PermissionsExt;

    let path = root.join("canberra-gtk-play");
    std::fs::write(&path, "#!/bin/sh\nexit 0\n").expect("write fake canberra tool");
    let mut permissions = std::fs::metadata(&path)
        .expect("fake canberra metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).expect("make fake canberra executable");
}

fn sound_name_hints(name: &str) -> HashMap<String, OwnedValue> {
    HashMap::from([(
        "sound-name".to_string(),
        Value::from(name)
            .try_into()
            .expect("sound name should convert"),
    )])
}

#[test]
fn playback_backend_requires_enabled_config_and_available_tool() {
    assert!(settings(true, SoundBackend::Canberra).has_playback_backend());
    assert!(!settings(false, SoundBackend::Canberra).has_playback_backend());
    assert!(!settings(true, SoundBackend::None).has_playback_backend());
}

#[test]
fn fdo_sound_capability_requires_allowed_file_hints_and_backend() {
    let mut sound = settings(true, SoundBackend::Canberra);
    assert!(!sound.supports_fdo_sound_capability());

    sound.allow_file_hints = true;
    sound.allowed_file_hint_dirs = vec![PathBuf::from("/allowed")];
    assert!(sound.supports_fdo_sound_capability());

    sound.backend = SoundBackend::None;
    assert!(!sound.supports_fdo_sound_capability());
}

#[test]
fn missing_backend_warning_policy_requires_enabled_sound_without_backend() {
    assert!(SoundSettings::should_warn_missing_backend(
        true,
        SoundBackend::None
    ));
    assert!(!SoundSettings::should_warn_missing_backend(
        false,
        SoundBackend::None
    ));
    assert!(!SoundSettings::should_warn_missing_backend(
        true,
        SoundBackend::Canberra
    ));
}

#[test]
fn default_source_prefers_file_before_event_name() {
    let root = TempRoot::new("sound-settings-default");
    let path = root.join("default.ogg");
    std::fs::write(&path, b"sound").expect("write default sound");
    let file = std::fs::File::open(&path).expect("open default sound");
    let mut sound = settings(true, SoundBackend::Canberra);
    sound.default_file = Some(SoundFile::new(path.clone(), file));

    match sound.default_source().expect("default source") {
        SoundSource::File(file) => assert_eq!(file.path(), path),
        SoundSource::Name(name) => panic!("file fallback should win over event name: {name}"),
    }

    sound.default_file = None;
    match sound.default_source().expect("default source") {
        SoundSource::Name(name) => assert_eq!(name, "message-new-instant"),
        SoundSource::File(path) => panic!("event name fallback should be used: {path:?}"),
    }

    sound.default_name = None;
    assert!(sound.default_source().is_none());
}

#[test]
fn play_from_hints_does_not_consume_throttle_when_global_or_notification_gate_blocks() {
    let disabled = settings(false, SoundBackend::Canberra);
    assert!(!disabled.play_from_hints(&HashMap::new(), true));
    assert!(!last_played_is_set(&disabled));

    let disallowed = settings(true, SoundBackend::Canberra);
    assert!(!disallowed.play_from_hints(&HashMap::new(), false));
    assert!(!last_played_is_set(&disallowed));

    let suppressed = settings(true, SoundBackend::Canberra);
    assert!(!suppressed.play_from_hints(&suppress_sound_hints(true), true));
    assert!(!last_played_is_set(&suppressed));
}

#[tokio::test(flavor = "current_thread")]
async fn play_from_hints_uses_default_source_and_records_allowed_attempt() {
    let root = TempRoot::new("sound-settings-play");
    install_fake_canberra(&root);
    let _tools = use_fake_tool_bin(root.path());
    let sound = settings(true, SoundBackend::Canberra);

    assert!(sound.play_from_hints(&HashMap::new(), true));

    assert!(last_played_is_set(&sound));
}

#[test]
fn play_from_hints_reports_false_when_no_backend_is_available() {
    let sound = settings(true, SoundBackend::None);

    assert!(!sound.play_from_hints(&HashMap::new(), true));
    assert!(!last_played_is_set(&sound));
}

#[test]
fn play_from_hints_returns_false_when_no_source_is_available() {
    let mut sound = settings(true, SoundBackend::None);
    sound.default_name = None;

    assert!(!sound.play_from_hints(&HashMap::new(), true));
    assert!(!last_played_is_set(&sound));
}

#[tokio::test(flavor = "current_thread")]
async fn accepted_playback_records_cooldown_and_throttles_immediate_repeat() {
    let root = TempRoot::new("sound-settings-cooldown");
    install_fake_canberra(&root);
    let _tools = use_fake_tool_bin(root.path());
    let sound = settings(true, SoundBackend::Canberra);

    assert!(sound.play_from_hints(&HashMap::new(), true));
    assert!(!sound.play_from_hints(&HashMap::new(), true));
}

#[tokio::test(flavor = "current_thread")]
async fn unusable_request_does_not_suppress_next_valid_sound() {
    let root = TempRoot::new("sound-settings-failed-then-valid");
    install_fake_canberra(&root);
    let _tools = use_fake_tool_bin(root.path());
    let mut sound = settings(true, SoundBackend::Canberra);
    sound.default_name = None;

    assert!(!sound.play_from_hints(&HashMap::new(), true));
    assert!(!last_played_is_set(&sound));
    assert!(sound.play_from_hints(&sound_name_hints("message-new"), true));
    assert!(last_played_is_set(&sound));
}

#[test]
fn play_reports_whether_backend_dispatch_was_available() {
    assert!(!settings(true, SoundBackend::PwPlay)
        .play(SoundSource::Name("message-new-instant".to_string())));
    assert!(!settings(true, SoundBackend::None)
        .play(SoundSource::Name("message-new-instant".to_string())));
}
