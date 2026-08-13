use clap::{CommandFactory, Parser};

use super::super::Args;

#[test]
fn root_help_lists_the_supported_command_groups() {
    let help = Args::command().render_help().to_string();

    assert!(help.contains("Usage:"));
    for command in [
        "open-panel",
        "dnd",
        "clear",
        "list-active",
        "doctor",
        "css-check",
        "preset",
        "theme",
    ] {
        assert!(help.contains(command), "missing {command} in {help}");
    }

    for internal_or_removed in [
        "dev",
        "refresh-applications",
        "explain-notification",
        "sync-session-environment",
        "clear-all",
    ] {
        assert!(
            !help.contains(internal_or_removed),
            "unexpected {internal_or_removed} in {help}"
        );
    }
}

#[test]
fn command_help_lists_user_facing_controls() {
    for (arguments, expected) in [
        (
            vec!["noticenterctl", "doctor", "--help"],
            vec![
                "repair-session",
                "--json",
                "--verbose",
                "--service-manager",
                "manual",
            ],
        ),
        (
            vec!["noticenterctl", "preset", "--help"],
            vec!["export", "import", "inspect", "reset-config"],
        ),
        (
            vec!["noticenterctl", "theme", "--help"],
            vec!["export-stock"],
        ),
    ] {
        let error = Args::try_parse_from(arguments).expect_err("help should stop parsing");
        let help = error.to_string();

        for value in expected {
            assert!(help.contains(value), "missing {value} in {help}");
        }
    }
}

#[test]
fn dev_help_lists_technical_commands_when_requested_explicitly() {
    let error = Args::try_parse_from(["noticenterctl", "dev", "--help"])
        .expect_err("help should stop parsing");
    let help = error.to_string();

    for command in [
        "open-panel",
        "refresh-applications",
        "explain-notification",
        "dump-active",
        "dump-history",
        "logs",
    ] {
        assert!(help.contains(command), "missing {command} in {help}");
    }
}

#[test]
fn normal_open_panel_help_has_no_diagnostic_options() {
    let error = Args::try_parse_from(["noticenterctl", "open-panel", "--help"])
        .expect_err("help should stop parsing");
    let help = error.to_string();

    assert!(!help.contains("debug"), "unexpected debug option in {help}");
    assert!(!help.contains("level"), "unexpected level option in {help}");
}

#[test]
fn invalid_commands_and_dnd_values_are_rejected_by_the_parser() {
    let command = Args::try_parse_from(["noticenterctl", "definitely-not-a-command"])
        .expect_err("unknown command should fail")
        .to_string();
    assert!(command.contains("unrecognized subcommand"));
    assert!(command.contains("definitely-not-a-command"));

    let dnd = Args::try_parse_from(["noticenterctl", "dnd", "maybe"])
        .expect_err("invalid DND state should fail")
        .to_string();
    assert!(dnd.contains("invalid value"));
    assert!(dnd.contains("maybe"));
}
