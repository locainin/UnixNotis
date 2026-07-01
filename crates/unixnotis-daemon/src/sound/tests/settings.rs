use super::*;
use zbus::zvariant::{OwnedValue, Value};

fn settings(enabled: bool, backend: SoundBackend) -> SoundSettings {
    SoundSettings {
        enabled,
        backend,
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

#[test]
fn supports_sound_requires_enabled_config_and_backend() {
    assert!(settings(true, SoundBackend::Canberra).supports_sound());
    assert!(!settings(false, SoundBackend::Canberra).supports_sound());
    assert!(!settings(true, SoundBackend::None).supports_sound());
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
    let mut sound = settings(true, SoundBackend::Canberra);
    sound.default_file = Some(PathBuf::from("/tmp/unixnotis-test.ogg"));

    match sound.default_source().expect("default source") {
        SoundSource::File(path) => assert_eq!(path, PathBuf::from("/tmp/unixnotis-test.ogg")),
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

#[test]
fn play_from_hints_uses_default_source_and_records_allowed_attempt() {
    let sound = settings(true, SoundBackend::PwPlay);

    assert!(sound.play_from_hints(&HashMap::new(), true));

    assert!(last_played_is_set(&sound));
}

#[test]
fn play_from_hints_reports_false_when_no_backend_is_available() {
    let sound = settings(true, SoundBackend::None);

    assert!(!sound.play_from_hints(&HashMap::new(), true));
    assert!(last_played_is_set(&sound));
}

#[test]
fn play_from_hints_returns_false_when_no_source_is_available() {
    let mut sound = settings(true, SoundBackend::None);
    sound.default_name = None;

    assert!(!sound.play_from_hints(&HashMap::new(), true));
    assert!(last_played_is_set(&sound));
}

#[test]
fn should_play_now_records_first_request_and_throttles_immediate_repeat() {
    let sound = settings(true, SoundBackend::Canberra);

    assert!(sound.should_play_now());
    assert!(!sound.should_play_now());
}

#[test]
fn should_play_now_accepts_when_last_play_is_older_than_interval() {
    let sound = settings(true, SoundBackend::Canberra);
    let now = Instant::now();
    *sound.last_played.lock().expect("last_played lock") = Some(now - SOUND_MIN_INTERVAL);

    assert!(sound.should_play_at(now));
}

#[test]
fn play_reports_whether_backend_dispatch_was_available() {
    assert!(settings(true, SoundBackend::PwPlay)
        .play(SoundSource::Name("message-new-instant".to_string())));
    assert!(!settings(true, SoundBackend::None)
        .play(SoundSource::Name("message-new-instant".to_string())));
}
