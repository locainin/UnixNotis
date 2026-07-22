use std::ffi::{OsStr, OsString};
use std::path::Path;

use super::super::CommandSpec;

#[test]
fn direct_spec_round_trips_through_toml_as_strings() {
    let spec =
        CommandSpec::direct("wpctl", ["get-volume", "@DEFAULT_AUDIO_SINK@"]).with_env("LANG", "C");
    let encoded = toml::to_string(&spec).expect("serialize direct command");
    let decoded: CommandSpec = toml::from_str(&encoded).expect("deserialize direct command");

    assert_eq!(decoded, spec);
    assert!(encoded.contains("mode = \"direct\""));
    assert!(encoded.contains("args = [\"get-volume\", \"@DEFAULT_AUDIO_SINK@\"]"));
}

#[test]
fn placeholder_replacement_preserves_direct_command_boundaries() {
    let spec = CommandSpec::direct("tool", ["--value={value}", "literal space"])
        .with_env("CURRENT", "{value}");
    let replaced = spec.replace("{value}", "42");

    assert_eq!(replaced.program(), Some(Path::new("tool")));
    assert_eq!(
        replaced.args(),
        Some(
            [
                OsString::from("--value=42"),
                OsString::from("literal space")
            ]
            .as_slice()
        )
    );
    assert_eq!(
        replaced
            .env()
            .expect("direct environment")
            .get(OsStr::new("CURRENT")),
        Some(&"42".into())
    );
}

#[test]
fn placeholder_replacement_updates_explicit_shell_script_without_reclassification() {
    let replaced = CommandSpec::shell("producer {value} | parser").replace("{value}", "7");

    assert_eq!(replaced, CommandSpec::shell("producer 7 | parser"));
}

#[test]
fn shell_detection_includes_direct_interpreter_invocations() {
    assert!(CommandSpec::shell("printf ready").uses_shell_command_string());
    for shell in ["sh", "ash", "bash", "dash", "fish", "ksh", "zsh"] {
        assert!(
            CommandSpec::direct(shell, ["-c", "printf ready"]).uses_shell_command_string(),
            "{shell} -c must retain the explicit shell boundary"
        );
    }
    assert!(CommandSpec::direct("/bin/bash", ["-lc", "printf ready"]).uses_shell_command_string());
    assert!(!CommandSpec::direct("sh", ["-x", "script"]).uses_shell_command_string());
    assert!(!CommandSpec::direct("printf", ["sh -c"]).uses_shell_command_string());
}

#[test]
fn shell_detection_does_not_treat_long_options_as_short_flag_clusters() {
    assert!(!CommandSpec::direct("bash", ["--norc", "script.sh"]).uses_shell_command_string());
    assert!(
        !CommandSpec::direct("fish", ["--no-config", "script.fish"]).uses_shell_command_string()
    );
}

#[test]
fn shell_detection_stops_at_option_and_script_boundaries() {
    assert!(!CommandSpec::direct("bash", ["--", "-c", "printf data"]).uses_shell_command_string());
    assert!(
        !CommandSpec::direct("bash", ["script.sh", "-c", "literal argument"])
            .uses_shell_command_string()
    );
}

#[test]
fn shell_detection_skips_option_values_before_command_flags() {
    for (shell, option, value) in [
        ("bash", "-O", "extglob"),
        ("sh", "-o", "nounset"),
        ("ash", "-o", "nounset"),
        ("dash", "-o", "nounset"),
        ("ksh", "-R", "restricted-root"),
        ("zsh", "-o", "SH_WORD_SPLIT"),
        ("fish", "--debug", "reader"),
    ] {
        assert!(
            CommandSpec::direct(shell, [option, value, "-c", "printf ready"])
                .uses_shell_command_string(),
            "{shell} must resume option scanning after the {option} value"
        );

        assert!(
            !CommandSpec::direct(shell, [option, "-c", "script.sh"]).uses_shell_command_string(),
            "{shell} must not interpret the {option} value as a command flag"
        );
    }

    assert!(CommandSpec::direct("bash", ["-x", "-c", "printf ready"]).uses_shell_command_string());
}

#[test]
fn fish_long_command_option_retains_the_command_string_boundary() {
    assert!(CommandSpec::direct("fish", ["--command=printf ready"]).uses_shell_command_string());
}

#[test]
fn command_accessors_distinguish_direct_and_shell_data() {
    let direct = CommandSpec::direct("printf", ["literal value"]);
    let shell = CommandSpec::shell("producer | parser");

    assert_eq!(direct.program(), Some(Path::new("printf")));
    assert_eq!(direct.script(), None);
    assert_eq!(shell.program(), None);
    assert_eq!(shell.args(), None);
    assert_eq!(shell.env(), None);
    assert_eq!(shell.script(), Some("producer | parser"));
}

#[test]
fn empty_commands_are_detected_in_both_explicit_modes() {
    assert!(CommandSpec::direct("", [] as [&str; 0]).is_empty());
    assert!(!CommandSpec::direct("printf", [] as [&str; 0]).is_empty());
    assert!(CommandSpec::shell(" \t\n").is_empty());
    assert!(!CommandSpec::shell("true").is_empty());
}

#[test]
fn command_display_keeps_program_arguments_and_shell_script_readable() {
    let direct = CommandSpec::direct("printf", ["literal value", "battery|charging"]);
    let shell = CommandSpec::shell("producer | parser");

    assert_eq!(
        direct.display_lossy(),
        "printf literal value battery|charging"
    );
    assert_eq!(direct.to_string(), "printf literal value battery|charging");
    assert_eq!(shell.display_lossy(), "producer | parser");
    assert_eq!(shell.to_string(), "producer | parser");
}
