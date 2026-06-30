use super::*;
use crate::test_support::{env_lock, EnvVarGuard, TempRoot};
use zbus::zvariant::Value;

fn string_value(value: &str) -> OwnedValue {
    Value::from(value)
        .try_into()
        .expect("string value should convert")
}

fn write_sound_file(path: &Path) {
    fs::write(path, b"sound").expect("write sound file");
}

#[test]
fn decode_file_uri_accepts_localhost_and_percent_decodes_path() {
    let uri = "file://localhost/tmp/sound%20file%3F.ogg";
    let expected = PathBuf::from("/tmp/sound file?.ogg");
    assert_eq!(decode_file_uri(uri), Some(expected));
}

#[test]
fn decode_file_uri_rejects_remote_hosts_relative_paths_and_bad_escapes() {
    assert!(decode_file_uri("file://example.com/tmp/sound.ogg").is_none());
    assert!(decode_file_uri("file://localhostrelative.ogg").is_none());
    assert!(decode_file_uri("file:///tmp/sound%zz.ogg").is_none());
}

#[test]
fn percent_decode_path_rejects_nul_and_keeps_utf8_valid() {
    // NUL is not valid inside filesystem paths
    assert!(percent_decode_path("/%00.wav").is_none());
    assert_eq!(
        percent_decode_path("/tmp/caf%C3%A9.wav").as_deref(),
        Some("/tmp/café.wav")
    );
}

#[test]
fn resolve_hint_sound_prefers_valid_sound_file_and_falls_back_to_name() {
    let root = TempRoot::new("sound-hints");
    let sound = root.join("alert.ogg");
    write_sound_file(&sound);

    let mut hints = HashMap::new();
    hints.insert(
        "sound-file".to_string(),
        string_value(&sound.to_string_lossy()),
    );
    hints.insert("sound-name".to_string(), string_value("message-new"));

    match resolve_hint_sound(&hints).expect("sound-file should resolve") {
        SoundSource::File(path) => assert_eq!(path, sound),
        SoundSource::Name(name) => panic!("sound file should win over name: {name}"),
    }

    hints.insert(
        "sound-file".to_string(),
        string_value("/missing/not-a-sound.ogg"),
    );
    match resolve_hint_sound(&hints).expect("sound-name should remain fallback") {
        SoundSource::Name(name) => assert_eq!(name, "message-new"),
        SoundSource::File(path) => panic!("invalid sound file should not be used: {path:?}"),
    }
}

#[test]
fn resolve_default_file_uses_relative_config_path_and_validates_file() {
    let _guard = env_lock();
    let root = TempRoot::new("sound-default-file");
    let config_dir = root.join("xdg");
    let unixnotis_dir = config_dir.join("unixnotis");
    fs::create_dir_all(&unixnotis_dir).expect("create config dir");
    let sound = unixnotis_dir.join("relative.ogg");
    write_sound_file(&sound);

    let _xdg = EnvVarGuard::set("XDG_CONFIG_HOME", &config_dir);

    let mut config = Config::default();
    config.sound.default_file = Some("relative.ogg".to_string());
    assert_eq!(resolve_default_file(&config), Some(sound));

    config.sound.default_file = Some("relative.txt".to_string());
    assert!(resolve_default_file(&config).is_none());
}

#[test]
fn choose_first_sound_file_filters_extensions_and_sorts_deterministically() {
    let root = TempRoot::new("sound-default-dir");
    fs::create_dir_all(root.join("a-dir.ogg")).expect("create directory decoy");
    write_sound_file(&root.join("z-last.wav"));
    write_sound_file(&root.join("b-first.OGG"));
    fs::write(root.join("ignored.txt"), b"not audio").expect("write ignored file");

    let selected = choose_first_sound_file(root.path()).expect("sound file should be selected");
    assert_eq!(
        selected.file_name().and_then(|name| name.to_str()),
        Some("b-first.OGG")
    );
}

#[test]
fn hint_bool_reads_only_boolean_hints() {
    let mut hints = HashMap::new();
    hints.insert("suppress-sound".to_string(), OwnedValue::from(true));
    hints.insert("not-bool".to_string(), string_value("true"));

    assert_eq!(hint_bool(&hints, "suppress-sound"), Some(true));
    assert_eq!(hint_bool(&hints, "not-bool"), None);
    assert_eq!(hint_bool(&hints, "missing"), None);
}

#[test]
fn max_sound_file_size_stays_at_sixteen_mib() {
    // This cap keeps notification-provided audio files from becoming large IO spikes
    assert_eq!(MAX_SOUND_FILE_BYTES, 16 * 1024 * 1024);
}

#[test]
fn has_audio_extension_accepts_supported_audio_extensions_only() {
    assert!(has_audio_extension(Path::new("sound.WAV")));
    assert!(has_audio_extension(Path::new("sound.flac")));
    assert!(!has_audio_extension(Path::new("sound.txt")));
    assert!(!has_audio_extension(Path::new("sound")));
}

#[test]
fn validate_sound_file_path_rejects_missing_oversized_and_non_audio_files() {
    let root = TempRoot::new("sound-validate");
    let valid = root.join("valid.ogg");
    let oversized = root.join("oversized.ogg");
    let wrong_ext = root.join("valid.txt");
    write_sound_file(&valid);
    write_sound_file(&wrong_ext);
    fs::File::create(&oversized)
        .expect("create oversized sound")
        .set_len(MAX_SOUND_FILE_BYTES + 1)
        .expect("resize oversized sound");

    assert!(validate_sound_file_path(&valid));
    assert!(!validate_sound_file_path(&oversized));
    assert!(!validate_sound_file_path(&wrong_ext));
    assert!(!validate_sound_file_path(&root.join("missing.ogg")));
}

#[cfg(target_os = "linux")]
#[test]
fn validate_sound_file_path_rejects_device_nodes() {
    assert!(!validate_sound_file_path(Path::new("/dev/zero")));
}
