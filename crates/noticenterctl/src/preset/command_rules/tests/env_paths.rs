use std::path::PathBuf;

use super::super::tokens::collect_outside_env_path_tokens;
use super::super::validate_command_paths_in_config_bytes;
use super::support::temp_root;

#[test]
fn validation_rejects_ld_preload_path_that_leaves_root() {
    let config_dir = temp_root("ld-preload-outside");
    let config = b"[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = \"LD_PRELOAD=/tmp/evil.so /bin/true\"\n";

    let error =
        validate_command_paths_in_config_bytes(&config_dir, config, "preset import blocked")
            .expect_err("reject LD_PRELOAD outside config root");

    assert!(error
        .to_string()
        .contains("points outside the UnixNotis config directory"));
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
        .contains("points outside the UnixNotis config directory"));
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
        .contains("points outside the UnixNotis config directory"));
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
        .contains("points outside the UnixNotis config directory"));
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
