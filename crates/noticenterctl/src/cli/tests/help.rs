use clap::{CommandFactory, Parser};

use super::super::Args;

#[test]
fn root_help_lists_the_supported_command_groups() {
    let help = Args::command().render_help().to_string();

    assert!(help.contains("Usage:"));
    assert!(help.contains("css-check"));
    assert!(help.contains("doctor"));
    assert!(help.contains("preset"));
    assert!(help.contains("theme"));
}

#[test]
fn command_help_lists_output_debug_and_preset_controls() {
    for (arguments, expected) in [
        (
            vec!["noticenterctl", "doctor", "--help"],
            vec!["--json", "--verbose", "--service-manager", "manual"],
        ),
        (
            vec!["noticenterctl", "open-panel", "--help"],
            vec!["--debug", "critical", "verbose"],
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
