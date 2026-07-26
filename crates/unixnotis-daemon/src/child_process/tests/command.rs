use std::ffi::OsString;
use std::path::{Path, PathBuf};

use clap::Parser;
use unixnotis_core::util::CONFIG_PATH_ENV;

use crate::test_support::daemon_state_for_test;

use super::{child_config_env_path, Args, UiProcessKind};

#[test]
fn ui_process_kind_labels_match_binary_names() {
    assert_eq!(UiProcessKind::Popups.label(), "unixnotis-popups");
    assert_eq!(UiProcessKind::Center.label(), "unixnotis-center");
}

#[tokio::test]
async fn mark_running_updates_popup_health_and_resets_center_readiness() {
    let state = daemon_state_for_test(false).await;

    UiProcessKind::Popups.mark_running(&state, true);
    assert!(state.popups_process_running());

    // Center process spawn is not readiness; readiness only flips after live subscriptions
    state.set_panel_ready(true);
    UiProcessKind::Center.mark_running(&state, true);
    assert!(!state.panel_ready());
}

#[test]
fn child_config_env_path_keeps_absolute_paths() {
    let path = Path::new("/tmp/unixnotis/config.toml");

    assert_eq!(child_config_env_path(path), PathBuf::from(path));
}

#[test]
fn child_config_env_path_resolves_relative_paths_against_current_directory() {
    let relative = Path::new("fixtures/config.toml");
    let expected = std::env::current_dir().expect("current dir").join(relative);

    assert_eq!(child_config_env_path(relative), expected);
}

#[test]
fn build_command_sets_config_env_instead_of_forwarding_flag() {
    let args = Args::parse_from(["unixnotis-daemon", "--config", "fixtures/config.toml"]);
    let command = UiProcessKind::build_command_for_path(
        &args,
        PathBuf::from("/tmp/unixnotis-test/bin/unixnotis-center"),
    );
    let std_command = command.as_std();
    let args: Vec<_> = std_command.get_args().map(OsString::from).collect();
    let envs: Vec<_> = std_command
        .get_envs()
        .filter_map(|(key, value)| value.map(|value| (key.to_owned(), value.to_owned())))
        .collect();

    assert!(
        !args.iter().any(|arg| arg == "--config"),
        "child argv should stay free of UnixNotis-only flags"
    );
    assert!(Path::new(std_command.get_program()).is_absolute());
    assert!(
        envs.iter().any(|(key, value)| {
            key == CONFIG_PATH_ENV
                && value == child_config_env_path(Path::new("fixtures/config.toml")).as_os_str()
        }),
        "custom config path should be handed to child apps by env"
    );
}

#[test]
fn build_command_clears_inherited_config_override_without_custom_path() {
    let args = Args::parse_from(["unixnotis-daemon"]);
    let command = UiProcessKind::build_command_for_path(
        &args,
        PathBuf::from("/tmp/unixnotis-test/bin/unixnotis-popups"),
    );
    let std_command = command.as_std();

    assert!(
        std_command
            .get_envs()
            .any(|(key, value)| key == CONFIG_PATH_ENV && value.is_none()),
        "default child launches should clear stale config overrides"
    );
}
