use super::*;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::test_support::test_env_lock;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    // PATH is process-global, so backend detection tests must not run over each other
    test_env_lock()
}

fn test_root(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    env::temp_dir().join(format!(
        "unixnotis-core-runtime-{name}-{}-{stamp}",
        std::process::id()
    ))
}

fn set_path(dir: &std::path::Path) -> Option<String> {
    let previous = env::var("PATH").ok();
    env::set_var("PATH", dir);
    previous
}

fn restore_path(previous: Option<String>) {
    match previous {
        Some(value) => env::set_var("PATH", value),
        None => env::remove_var("PATH"),
    }
}

fn write_fake_program(dir: &std::path::Path, name: &str) {
    let path = dir.join(name);
    fs::write(&path, "#!/bin/sh\nexit 0\n").expect("fake program should be writable");

    #[cfg(unix)]
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
        .expect("fake program should be executable");
}

#[test]
fn custom_volume_without_watch_stays_config_owned() {
    let mut volume = SliderWidgetConfig {
        enabled: true,
        label: "Volume".to_string(),
        icon: "audio-volume-high-symbolic".to_string(),
        icon_muted: None,
        get_cmd: CommandSpec::direct("custom-volume-get", [] as [&str; 0]),
        set_cmd: CommandSpec::direct("custom-volume-set", ["{value}"]),
        toggle_cmd: None,
        watch_cmd: None,
        min: 0.0,
        max: 100.0,
        step: 1.0,
        show_value: true,
        segments: 0,
        show_sublabels: false,
        sublabel_min: String::new(),
        sublabel_max: String::new(),
        parse_mode: NumericParseMode::Auto,
    };

    apply_volume_backend(&mut volume);

    assert!(volume.watch_cmd.is_none());
}

#[test]
fn partial_stock_volume_commands_do_not_migrate_to_pactl() {
    let _guard = env_lock();
    let root = test_root("partial-stock-volume");
    fs::create_dir_all(&root).expect("fake bin dir");
    write_fake_program(&root, "pactl");
    let previous = set_path(&root);

    let cases = [
        SliderWidgetConfig {
            get_cmd: CommandSpec::direct("custom", ["get"]),
            ..SliderWidgetConfig::default()
        },
        SliderWidgetConfig {
            set_cmd: CommandSpec::direct("custom", ["set", "{value}"]),
            ..SliderWidgetConfig::default()
        },
        SliderWidgetConfig {
            toggle_cmd: Some(CommandSpec::direct("custom", ["toggle"])),
            ..SliderWidgetConfig::default()
        },
    ];

    for mut volume in cases {
        let original_get = volume.get_cmd.clone();
        let original_set = volume.set_cmd.clone();
        let original_toggle = volume.toggle_cmd.clone();
        let original_watch = volume.watch_cmd.clone();
        apply_volume_backend(&mut volume);

        assert_eq!(volume.get_cmd, original_get);
        assert_eq!(volume.set_cmd, original_set);
        assert_eq!(volume.toggle_cmd, original_toggle);
        assert_eq!(volume.watch_cmd, original_watch);
    }

    restore_path(previous);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn stock_volume_uses_pactl_when_wpctl_is_missing() {
    let _guard = env_lock();
    let root = test_root("pactl-only");
    fs::create_dir_all(&root).expect("fake bin dir");
    write_fake_program(&root, "pactl");
    let previous = set_path(&root);

    let mut volume = SliderWidgetConfig::default();
    apply_volume_backend(&mut volume);

    assert!(volume.enabled);
    assert_eq!(volume.get_cmd, SliderWidgetConfig::pactl_get());
    assert_eq!(volume.set_cmd, SliderWidgetConfig::pactl_set());
    assert_eq!(volume.toggle_cmd, Some(SliderWidgetConfig::pactl_toggle()));
    assert_eq!(volume.watch_cmd, Some(SliderWidgetConfig::pactl_watch()));

    restore_path(previous);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn stock_volume_keeps_wpctl_when_available() {
    let _guard = env_lock();
    let root = test_root("wpctl-and-pactl");
    fs::create_dir_all(&root).expect("fake bin dir");
    write_fake_program(&root, "wpctl");
    write_fake_program(&root, "pactl");
    let previous = set_path(&root);

    let mut volume = SliderWidgetConfig::default();
    apply_volume_backend(&mut volume);

    assert!(volume.enabled);
    assert_eq!(volume.get_cmd, SliderWidgetConfig::wpctl_get());
    assert_eq!(volume.set_cmd, SliderWidgetConfig::wpctl_set());
    assert_eq!(volume.watch_cmd, Some(SliderWidgetConfig::pactl_watch()));

    restore_path(previous);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn stock_volume_disables_when_no_supported_backend_exists() {
    let _guard = env_lock();
    let root = test_root("no-audio-backend");
    fs::create_dir_all(&root).expect("fake bin dir");
    let previous = set_path(&root);

    let mut volume = SliderWidgetConfig::default();
    apply_volume_backend(&mut volume);

    assert!(!volume.enabled);

    restore_path(previous);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn legacy_wpctl_watch_is_removed_when_pactl_is_missing() {
    let _guard = env_lock();
    let root = test_root("legacy-watch-no-pactl");
    fs::create_dir_all(&root).expect("fake bin dir");
    write_fake_program(&root, "wpctl");
    let previous = set_path(&root);

    let mut volume = SliderWidgetConfig {
        watch_cmd: Some(CommandSpec::direct("wpctl", ["subscribe"])),
        ..SliderWidgetConfig::default()
    };
    apply_volume_backend(&mut volume);

    assert_eq!(volume.watch_cmd, None);

    restore_path(previous);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn legacy_brightness_watch_is_removed() {
    let mut brightness = SliderWidgetConfig {
        enabled: true,
        label: "Brightness".to_string(),
        icon: "display-brightness-symbolic".to_string(),
        icon_muted: None,
        get_cmd: CommandSpec::direct("brightnessctl", ["-m"]),
        set_cmd: CommandSpec::direct("brightnessctl", ["s", "{value}%"]),
        toggle_cmd: None,
        watch_cmd: Some(CommandSpec::direct("brightnessctl", ["-w"])),
        min: 1.0,
        max: 100.0,
        step: 1.0,
        show_value: true,
        segments: 0,
        show_sublabels: false,
        sublabel_min: String::new(),
        sublabel_max: String::new(),
        parse_mode: NumericParseMode::Auto,
    };

    apply_brightness_backend(&mut brightness);

    assert!(brightness.watch_cmd.is_none());
}
