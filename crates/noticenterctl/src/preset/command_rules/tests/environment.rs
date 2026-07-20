use std::path::PathBuf;

use super::super::tokens::{collect_outside_env_path_tokens, validate_env_path_semantics};
use super::support::{parsed_command, temp_root};
use unixnotis_core::{parse_legacy_command as parse_command, CommandSpec};

#[test]
fn env_path_token_collector_finds_ld_preload_outside_root() {
    let config_dir = temp_root("ld-preload-token");

    let outside = collect_outside_env_path_tokens(
        &config_dir,
        &parsed_command("LD_PRELOAD=/tmp/evil.so /bin/true"),
    );

    assert_eq!(outside.len(), 1);
    assert_eq!(outside[0].0, "LD_PRELOAD");
    assert_eq!(outside[0].1, PathBuf::from("/tmp/evil.so"));
}

#[test]
fn validation_rejects_pythonhome_empty_or_ambiguous_prefixes() {
    for (command, reason) in [
        (
            "PYTHONHOME=':exec-runtime' python3 -c pass",
            "PYTHONHOME contains an empty prefix",
        ),
        (
            "PYTHONHOME='runtime:' python3 -c pass",
            "PYTHONHOME contains an empty prefix",
        ),
        (
            "PYTHONHOME='a:b:c' python3 -c pass",
            "PYTHONHOME contains more than one prefix separator",
        ),
    ] {
        let parsed = parse_command(command).expect("parse PYTHONHOME command");

        assert_eq!(
            validate_env_path_semantics(&parsed),
            Err(reason),
            "wrong PYTHONHOME result for {command}"
        );
    }
}

#[test]
fn validation_rejects_dynamic_loader_tokens_and_ambiguous_bare_objects() {
    for command in [
        "LD_PRELOAD='$ORIGIN/libevil.so' /bin/true",
        "LD_LIBRARY_PATH='${LIB}' /bin/true",
        "LD_AUDIT='$PLATFORM/audit.so' /bin/true",
        "LD_PRELOAD=libprobe.so /bin/true",
        "LD_AUDIT=audit.so /bin/true",
    ] {
        let parsed = parse_command(command).expect("parse loader environment command");
        assert!(
            validate_env_path_semantics(&parsed).is_err(),
            "unsafe loader value was accepted: {command}"
        );
    }
}

#[test]
fn validation_rejects_shell_startup_path_expansions() {
    for command in [
        "BASH_ENV='$HOME/evil' /bin/true",
        "ENV='$(touch marker)' /bin/true",
        "BASH_ENV='~/evil' /bin/true",
    ] {
        let parsed = parse_command(command).expect("parse shell environment command");
        assert!(
            validate_env_path_semantics(&parsed).is_err(),
            "expanded shell startup path was accepted: {command}"
        );
    }
}

#[test]
fn env_path_token_collector_ignores_invalid_env_assignment_names() {
    let config_dir = temp_root("invalid-env-token");

    let outside =
        collect_outside_env_path_tokens(&config_dir, &parsed_command("/tmp/with=equals /bin/true"));

    assert!(outside.is_empty());
}

#[test]
fn env_path_token_collector_ignores_commands_with_carriage_returns() {
    let config_dir = temp_root("carriage-return-env-token");

    let outside = collect_outside_env_path_tokens(
        &config_dir,
        &CommandSpec::shell("LD_PRELOAD=/tmp/evil.so\r/bin/true"),
    );

    assert!(outside.is_empty());
}

#[test]
fn env_path_token_collector_ignores_unknown_env_names() {
    let config_dir = temp_root("unknown-env-token");

    let outside = collect_outside_env_path_tokens(
        &config_dir,
        &parsed_command("WIDGET_DATA=/tmp/evil /bin/true"),
    );

    assert!(outside.is_empty());
}

#[test]
fn validation_ignores_loader_tokens_in_unknown_environment_variables() {
    let parsed = parse_command("WIDGET_DATA='$ORIGIN/data' scripts/probe")
        .expect("parse unknown environment variable");

    validate_env_path_semantics(&parsed)
        .expect("unknown variables do not use loader path semantics");
}

#[test]
fn env_path_token_collector_fails_closed_for_shell_assignment_scope() {
    let config_dir = temp_root("complex-env-token");

    let outside = collect_outside_env_path_tokens(
        &config_dir,
        &CommandSpec::direct("/bin/true", [] as [&str; 0]).with_env("LD_PRELOAD", "/tmp/evil.so"),
    );

    assert_eq!(outside.len(), 1);
    assert_eq!(outside[0].0, "LD_PRELOAD");
}
