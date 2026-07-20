use super::super::tokens::{
    first_command_token, split_env_assignment, validate_env_command_layout,
};
use super::super::validate_command_paths_in_config_bytes;
use super::support::{parsed_command, temp_root};
use unixnotis_core::parse_legacy_command as parse_command;

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
            first_command_token(&parsed_command(command)).as_deref(),
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
            first_command_token(&parsed_command(command)).as_deref(),
            Some("scripts/probe"),
            "assignment range consumed the wrong child for {command}"
        );
    }

    assert_eq!(
        first_command_token(&parsed_command("env MODE=safe LEVEL=2")),
        None
    );
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
            first_command_token(&parsed_command(command)),
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
            first_command_token(&parsed_command(command)),
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
