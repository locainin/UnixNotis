use super::*;
use crate::{
    CardWidgetConfig, Config, SliderWidgetConfig, StatWidgetConfig, ToggleWidgetConfig,
    WidgetPluginConfig,
};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::test_support::test_env_lock;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    // PATH changes are process-global and affect program discovery cache tests
    test_env_lock()
}

fn test_root(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    env::temp_dir().join(format!(
        "unixnotis-core-shell-{name}-{}-{stamp}",
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
    fs::write(&path, "#!/bin/sh\nexit 0\n").expect("fake program");

    #[cfg(unix)]
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("fake executable");
}

fn config_without_shell_commands() -> Config {
    let mut config = Config::default();
    config.widgets.volume.get_cmd = CommandSpec::direct("volume-get", [] as [&str; 0]);
    config.widgets.volume.set_cmd = CommandSpec::direct("volume-set", ["{value}"]);
    config.widgets.volume.toggle_cmd = None;
    config.widgets.volume.watch_cmd = None;
    config.widgets.brightness.get_cmd = CommandSpec::direct("brightness-get", [] as [&str; 0]);
    config.widgets.brightness.set_cmd = CommandSpec::direct("brightness-set", ["{value}"]);
    config.widgets.brightness.toggle_cmd = None;
    config.widgets.brightness.watch_cmd = None;
    config.widgets.toggles.clear();
    config.widgets.stats.clear();
    config.widgets.cards.clear();
    config
}

#[test]
fn command_requires_shell_accepts_plain_commands_and_placeholders() {
    assert!(!command_requires_shell(&CommandSpec::direct(
        "notify-send",
        ["hello"]
    )));
    assert!(!command_requires_shell(&CommandSpec::direct(
        "wpctl",
        ["set-volume", "sink", "{value}%"]
    )));
    assert!(!command_requires_shell(&CommandSpec::direct(
        "",
        [] as [&str; 0]
    )));
}

#[test]
fn command_requires_shell_rejects_shell_syntax() {
    for script in [
        "echo hi | wc -l",
        "echo hi && echo bye",
        "echo > file",
        "echo $(date)",
        "echo ~/file",
    ] {
        assert!(
            command_requires_shell(&CommandSpec::shell(script)),
            "command should need shell: {script}"
        );
    }
}

#[test]
fn optional_command_requires_shell_only_when_present_and_complex() {
    assert!(!command_requires_shell_opt(&None));
    assert!(!command_requires_shell_opt(&Some(CommandSpec::direct(
        "notify-send",
        ["hi"]
    ))));
    assert!(command_requires_shell_opt(&Some(CommandSpec::shell(
        "notify-send hi | cat"
    ))));
}

#[test]
fn config_requires_shell_checks_volume_and_brightness_commands() {
    let mut config = Config::default();
    assert!(config_requires_shell(&Config {
        widgets: crate::WidgetsConfig {
            volume: SliderWidgetConfig {
                get_cmd: CommandSpec::shell("echo volume | cat"),
                ..config.widgets.volume.clone()
            },
            ..config.widgets.clone()
        },
        ..config.clone()
    }));

    config.widgets.brightness.set_cmd =
        CommandSpec::shell("brightnessctl s {value}% && notify-send done");
    assert!(config_requires_shell(&config));
}

#[test]
fn config_requires_shell_checks_each_slider_command_branch() {
    let slider_cases: [fn(&mut Config); 8] = [
        |config: &mut Config| config.widgets.volume.get_cmd = CommandSpec::shell("echo get | cat"),
        |config: &mut Config| config.widgets.volume.set_cmd = CommandSpec::shell("echo set | cat"),
        |config: &mut Config| {
            config.widgets.volume.toggle_cmd = Some(CommandSpec::shell("echo toggle | cat"));
        },
        |config: &mut Config| {
            config.widgets.volume.watch_cmd = Some(CommandSpec::shell("echo watch | cat"));
        },
        |config: &mut Config| {
            config.widgets.brightness.get_cmd = CommandSpec::shell("echo bget | cat");
        },
        |config: &mut Config| {
            config.widgets.brightness.set_cmd = CommandSpec::shell("echo bset | cat");
        },
        |config: &mut Config| {
            config.widgets.brightness.toggle_cmd = Some(CommandSpec::shell("echo btoggle | cat"));
        },
        |config: &mut Config| {
            config.widgets.brightness.watch_cmd = Some(CommandSpec::shell("echo bwatch | cat"));
        },
    ];

    for make_shell_command in slider_cases {
        let mut config = Config::default();
        config.widgets.toggles.clear();
        config.widgets.stats.clear();
        config.widgets.cards.clear();
        make_shell_command(&mut config);

        assert!(config_requires_shell(&config));
    }
}

#[test]
fn config_requires_shell_checks_toggle_commands() {
    let mut config = Config::default();
    config.widgets.toggles = vec![ToggleWidgetConfig {
        toggle_cmd: Some(CommandSpec::shell("echo toggle | cat")),
        ..ToggleWidgetConfig::default()
    }];

    assert!(config_requires_shell(&config));
}

#[test]
fn config_requires_shell_checks_each_toggle_command_branch() {
    let toggle_cases: [fn(&mut ToggleWidgetConfig); 5] = [
        |toggle: &mut ToggleWidgetConfig| {
            toggle.state_cmd = Some(CommandSpec::shell("echo state | cat"));
        },
        |toggle: &mut ToggleWidgetConfig| {
            toggle.toggle_cmd = Some(CommandSpec::shell("echo toggle | cat"));
        },
        |toggle: &mut ToggleWidgetConfig| toggle.on_cmd = Some(CommandSpec::shell("echo on | cat")),
        |toggle: &mut ToggleWidgetConfig| {
            toggle.off_cmd = Some(CommandSpec::shell("echo off | cat"));
        },
        |toggle: &mut ToggleWidgetConfig| {
            toggle.watch_cmd = Some(CommandSpec::shell("echo watch | cat"));
        },
    ];

    for make_shell_command in toggle_cases {
        let mut toggle = ToggleWidgetConfig::default();
        make_shell_command(&mut toggle);
        let mut config = Config::default();
        config.widgets.toggles = vec![toggle];
        config.widgets.stats.clear();
        config.widgets.cards.clear();

        assert!(config_requires_shell(&config));
    }
}

#[test]
fn config_requires_shell_checks_stat_and_card_commands_and_plugins() {
    let mut stat_config = config_without_shell_commands();
    stat_config.widgets.stats = vec![StatWidgetConfig {
        cmd: Some(CommandSpec::shell("echo stat | cat")),
        ..StatWidgetConfig::default()
    }];
    assert!(config_requires_shell(&stat_config));

    let mut stat_plugin = config_without_shell_commands();
    stat_plugin.widgets.stats = vec![StatWidgetConfig {
        plugin: Some(WidgetPluginConfig {
            command: CommandSpec::shell("echo stat-plugin | cat"),
            ..WidgetPluginConfig::default()
        }),
        ..StatWidgetConfig::default()
    }];
    assert!(config_requires_shell(&stat_plugin));

    let mut card_config = config_without_shell_commands();
    card_config.widgets.cards = vec![CardWidgetConfig {
        cmd: Some(CommandSpec::shell("echo card | cat")),
        ..CardWidgetConfig::default()
    }];
    assert!(config_requires_shell(&card_config));

    let mut card_plugin = config_without_shell_commands();
    card_plugin.widgets.cards = vec![CardWidgetConfig {
        plugin: Some(WidgetPluginConfig {
            command: CommandSpec::shell("echo card-plugin | cat"),
            ..WidgetPluginConfig::default()
        }),
        ..CardWidgetConfig::default()
    }];
    assert!(config_requires_shell(&card_plugin));
}

#[test]
fn warn_missing_shell_reports_only_when_shell_is_missing_and_needed() {
    let _guard = env_lock();
    let root = test_root("missing-sh");
    let with_sh = test_root("with-sh");
    fs::create_dir_all(&root).expect("fake bin dir");
    fs::create_dir_all(&with_sh).expect("fake sh dir");
    let previous = set_path(&root);
    let mut config = Config::default();
    config.widgets.toggles = vec![ToggleWidgetConfig {
        toggle_cmd: Some(CommandSpec::shell("echo toggle | cat")),
        ..ToggleWidgetConfig::default()
    }];
    config.widgets.stats.clear();
    config.widgets.cards.clear();

    assert!(warn_missing_shell(&config));

    write_fake_program(&with_sh, "sh");
    env::set_var("PATH", &with_sh);
    assert!(!warn_missing_shell(&config));

    restore_path(previous);
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(with_sh);
}

#[test]
fn config_requires_shell_stays_false_for_stock_config() {
    let config = config_without_shell_commands();

    assert!(!config_requires_shell(&config));
}
