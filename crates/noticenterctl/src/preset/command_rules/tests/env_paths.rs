use std::path::PathBuf;

use super::super::tokens::{
    collect_outside_env_path_tokens, first_command_token, is_host_specific_path_token,
    looks_like_path_token, split_env_assignment, validate_env_command_layout,
};
use super::super::validate_command_paths_in_config_bytes;
use super::support::temp_root;
use unixnotis_core::parse_command;

#[test]
fn validation_rejects_ld_preload_path_that_leaves_root() {
    let config_dir = temp_root("ld-preload-outside");
    let config = b"[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = \"LD_PRELOAD=/tmp/evil.so /bin/true\"\n";

    let error =
        validate_command_paths_in_config_bytes(&config_dir, config, "preset import blocked")
            .expect_err("reject LD_PRELOAD outside config root");

    assert!(error
        .to_string()
        .contains("resolves outside the UnixNotis config directory"));
}

#[test]
fn validation_rejects_quoted_ld_preload_paths_that_leave_root() {
    let config_dir = temp_root("quoted-ld-preload-outside");
    for command in [
        "LD_PRELOAD=\"/tmp/evil.so\" /bin/true",
        "LD_PRELOAD='/tmp/evil.so' /bin/true",
        "env LD_PRELOAD=/tmp/evil.so /bin/true",
    ] {
        let config = format!(
            "[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = {command:?}\n"
        );
        validate_command_paths_in_config_bytes(
            &config_dir,
            config.as_bytes(),
            "preset import blocked",
        )
        .expect_err("reject quoted or env-wrapped preload escape");
    }
}

#[test]
fn validation_rejects_tilde_program_and_malformed_quoting() {
    let config_dir = temp_root("tilde-and-quote");
    let tilde = b"[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = \"~/outside-script\"\n";
    validate_command_paths_in_config_bytes(&config_dir, tilde, "preset import blocked")
        .expect_err("reject tilde program outside config root");

    let malformed = b"[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = 'echo \"unterminated'\n";
    validate_command_paths_in_config_bytes(&config_dir, malformed, "preset import blocked")
        .expect_err("reject malformed command quoting");
}

#[test]
fn validation_rejects_home_override_and_env_wrapped_absolute_program() {
    let config_dir = temp_root("home-and-env-program");
    for command in ["HOME=/tmp ./script", "env SAFE=value /bin/true"] {
        let config = format!(
            "[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = {command:?}\n"
        );
        validate_command_paths_in_config_bytes(
            &config_dir,
            config.as_bytes(),
            "preset import blocked",
        )
        .expect_err("reject path policy escape");
    }
}

#[test]
fn env_path_token_collector_finds_ld_preload_outside_root() {
    let config_dir = temp_root("ld-preload-token");

    let outside = collect_outside_env_path_tokens(&config_dir, "LD_PRELOAD=/tmp/evil.so /bin/true");

    assert_eq!(outside.len(), 1);
    assert_eq!(outside[0].0, "LD_PRELOAD");
    assert_eq!(outside[0].1, PathBuf::from("/tmp/evil.so"));
}

#[test]
fn env_path_token_collector_ignores_invalid_env_assignment_names() {
    let config_dir = temp_root("invalid-env-token");

    let outside = collect_outside_env_path_tokens(&config_dir, "/tmp/with=equals /bin/true");

    assert!(outside.is_empty());
}

#[test]
fn env_path_token_collector_ignores_commands_with_carriage_returns() {
    let config_dir = temp_root("carriage-return-env-token");

    let outside =
        collect_outside_env_path_tokens(&config_dir, "LD_PRELOAD=/tmp/evil.so\r/bin/true");

    assert!(outside.is_empty());
}

#[test]
fn env_path_token_collector_ignores_unknown_env_names() {
    let config_dir = temp_root("unknown-env-token");

    let outside = collect_outside_env_path_tokens(&config_dir, "WIDGET_DATA=/tmp/evil /bin/true");

    assert!(outside.is_empty());
}

#[test]
fn env_path_token_collector_defers_shell_assignment_scope_to_exec_review() {
    let config_dir = temp_root("complex-env-token");

    let outside =
        collect_outside_env_path_tokens(&config_dir, "LD_PRELOAD=/tmp/evil.so; /bin/true");

    assert!(outside.is_empty());
}

#[test]
fn env_path_token_collector_ignores_bare_library_names() {
    let config_dir = temp_root("bare-env-token");

    let outside = collect_outside_env_path_tokens(&config_dir, "LD_PRELOAD=libprobe.so /bin/true");

    assert!(outside.is_empty());
}

#[test]
fn validation_rejects_colon_separated_env_path_that_leaves_root() {
    let config_dir = temp_root("pythonpath-outside");
    let config = b"[theme]\nbase_css = \"base.css\"\n[[widgets.cards]]\nlabel = \"Probe\"\ncmd = \"PYTHONPATH=scripts:/tmp/evil python3 -c pass\"\n";

    let error =
        validate_command_paths_in_config_bytes(&config_dir, config, "preset import blocked")
            .expect_err("reject PYTHONPATH outside config root");

    assert!(error
        .to_string()
        .contains("resolves outside the UnixNotis config directory"));
}

#[test]
fn validation_accepts_dangerous_env_paths_inside_root() {
    let config_dir = temp_root("env-path-inside");
    let config = b"[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = \"LD_PRELOAD=scripts/libprobe.so scripts/probe\"\n";

    validate_command_paths_in_config_bytes(&config_dir, config, "preset import blocked")
        .expect("config-root-relative env paths should be allowed");
}

#[test]
fn validation_does_not_mistake_env_option_values_for_the_child_program() {
    let config_dir = temp_root("env-option-program");
    let config = b"[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = \"env -u HOME /tmp/outside-probe\"\n";

    let error =
        validate_command_paths_in_config_bytes(&config_dir, config, "preset import blocked")
            .expect_err("reject external child after env option value");

    assert!(error
        .to_string()
        .contains("resolves outside the UnixNotis config directory"));
}

#[test]
fn validation_checks_env_assignments_that_follow_options() {
    let config_dir = temp_root("env-option-assignment");
    let config = b"[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = \"env -i LD_PRELOAD=/tmp/evil.so scripts/probe\"\n";

    let error =
        validate_command_paths_in_config_bytes(&config_dir, config, "preset import blocked")
            .expect_err("reject external environment path after env option");

    assert!(error
        .to_string()
        .contains("resolves outside the UnixNotis config directory"));
}

#[test]
fn validation_rejects_nonportable_env_reinterpretation_options() {
    let config_dir = temp_root("env-nonportable-options");
    for command in [
        "env -C scripts ./probe",
        "env --chdir=scripts ./probe",
        "env -S 'MODE=safe /tmp/outside-probe'",
        "env --split-string='MODE=safe /tmp/outside-probe'",
    ] {
        let config = format!(
            "[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = {command:?}\n"
        );
        let error = validate_command_paths_in_config_bytes(
            &config_dir,
            config.as_bytes(),
            "preset import blocked",
        )
        .expect_err("reject nonportable env option");

        assert!(error.to_string().contains("unsafe env wrapper"));
    }
}

#[test]
fn validation_accepts_supported_env_options_before_a_portable_program() {
    let config_dir = temp_root("env-supported-options");
    let config = b"[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = \"env -iv -u HOME MODE=safe scripts/probe\"\n";

    validate_command_paths_in_config_bytes(&config_dir, config, "preset import blocked")
        .expect("supported env options should preserve child discovery");
}

#[test]
fn every_supported_env_option_preserves_the_real_child_program() {
    for command in [
        "env -- scripts/probe",
        "env - scripts/probe",
        "env -i scripts/probe",
        "env -0 scripts/probe",
        "env -v scripts/probe",
        "env --ignore-environment scripts/probe",
        "env --null scripts/probe",
        "env --debug scripts/probe",
        "env --list-signal-handling scripts/probe",
        "env -u HOME scripts/probe",
        "env --unset HOME scripts/probe",
        "env -a probe scripts/probe",
        "env --argv0 probe scripts/probe",
        "env -uHOME scripts/probe",
        "env --unset=HOME scripts/probe",
        "env -aprobe scripts/probe",
        "env --argv0=probe scripts/probe",
        "env --block-signal scripts/probe",
        "env --block-signal=PIPE scripts/probe",
        "env --default-signal scripts/probe",
        "env --default-signal=PIPE scripts/probe",
        "env --ignore-signal scripts/probe",
        "env --ignore-signal=PIPE scripts/probe",
        "env -iv0 scripts/probe",
    ] {
        assert_eq!(
            first_command_token(command).as_deref(),
            Some("scripts/probe"),
            "wrong env child for {command}"
        );
    }
}

#[test]
fn env_layout_counts_assignments_after_every_option() {
    for command in [
        "env -- MODE=safe LEVEL=2 scripts/probe",
        "env -iv0 MODE=safe LEVEL=2 scripts/probe",
        "env --unset=HOME MODE=safe LEVEL=2 scripts/probe",
        "env --block-signal=PIPE MODE=safe LEVEL=2 scripts/probe",
    ] {
        assert_eq!(
            first_command_token(command).as_deref(),
            Some("scripts/probe"),
            "assignment range consumed the wrong child for {command}"
        );
    }

    assert_eq!(first_command_token("env MODE=safe LEVEL=2"), None);
}

#[test]
fn unsupported_and_incomplete_env_options_never_become_child_programs() {
    for command in [
        "env -u",
        "env --unset",
        "env -a",
        "env --argv0",
        "env --unknown scripts/probe",
        "env -ix scripts/probe",
    ] {
        assert_eq!(
            first_command_token(command),
            None,
            "unsafe env layout was accepted for {command}"
        );
    }
}

#[test]
fn every_nonportable_env_option_form_is_rejected() {
    for command in [
        "env -C scripts scripts/probe",
        "env -Cscripts scripts/probe",
        "env --chdir scripts scripts/probe",
        "env --chdir=scripts scripts/probe",
        "env -S scripts/probe",
        "env -SMODE=safe scripts/probe",
        "env --split-string scripts/probe",
        "env --split-string=MODE=safe scripts/probe",
    ] {
        assert_eq!(
            first_command_token(command),
            None,
            "nonportable env layout was accepted for {command}"
        );
    }
}

#[test]
fn nonportable_env_options_keep_specific_actionable_reasons() {
    for command in [
        "env -C scripts scripts/probe",
        "env -Cscripts scripts/probe",
        "env --chdir scripts scripts/probe",
        "env --chdir=scripts scripts/probe",
    ] {
        let parsed = parse_command(command).expect("parse env command");
        assert_eq!(
            validate_env_command_layout(&parsed),
            Err("env working-directory options are not portable in preset commands"),
            "wrong working-directory reason for {command}"
        );
    }

    for command in [
        "env -S scripts/probe",
        "env -SMODE=safe scripts/probe",
        "env --split-string scripts/probe",
        "env --split-string=MODE=safe scripts/probe",
    ] {
        let parsed = parse_command(command).expect("parse env command");
        assert_eq!(
            validate_env_command_layout(&parsed),
            Err("env split-string options are ambiguous in preset commands"),
            "wrong split-string reason for {command}"
        );
    }
}

#[test]
fn env_assignment_names_follow_portable_shell_identifier_rules() {
    assert_eq!(split_env_assignment("NAME=value"), Some(("NAME", "value")));
    assert_eq!(split_env_assignment("_NAME=a=b"), Some(("_NAME", "a=b")));
    assert_eq!(split_env_assignment("A1="), Some(("A1", "")));

    for token in [
        "1NAME=value",
        "-NAME=value",
        "NA-ME=value",
        "=value",
        "NAME",
    ] {
        assert_eq!(
            split_env_assignment(token),
            None,
            "invalid assignment name accepted for {token}"
        );
    }
}

#[test]
fn path_token_detection_covers_every_supported_relative_form() {
    for token in ["~", "~/tool", "./tool", "../tool", "dir/tool", "/tool"] {
        assert!(
            looks_like_path_token(token),
            "path form not detected: {token}"
        );
    }
    for token in ["", "tool", "tool-name", ".", ".."] {
        assert!(
            !looks_like_path_token(token),
            "plain command was treated as a path: {token}"
        );
    }
}

#[test]
fn host_specific_path_detection_excludes_portable_relative_paths() {
    for token in ["/usr/bin/tool", "~", "~/bin/tool"] {
        assert!(
            is_host_specific_path_token(token),
            "host path not detected: {token}"
        );
    }
    for token in ["tool", "./tool", "../tool", "dir/tool"] {
        assert!(
            !is_host_specific_path_token(token),
            "portable path was treated as host-specific: {token}"
        );
    }
}
