use std::env;

use super::super::journal::{
    daemon_unit_from_env, follow_args, probe_args, recent_args, validate_daemon_unit,
};

#[test]
fn daemon_unit_from_env_uses_override_when_present() {
    let unit = daemon_unit_from_env(|key| {
        assert_eq!(key, "UNIXNOTIS_DAEMON_UNIT");
        Ok("custom.service".to_string())
    });

    assert_eq!(unit.expect("valid custom unit"), "custom.service");
}

#[test]
fn daemon_unit_from_env_uses_default_when_override_is_absent() {
    let unit = daemon_unit_from_env(|_| Err(env::VarError::NotPresent));

    assert_eq!(
        unit.expect("valid default unit"),
        "unixnotis-daemon.service"
    );
}

#[test]
fn follow_args_target_user_unit_and_stream_plain_messages() {
    assert_eq!(
        follow_args("custom.service"),
        vec!["--user", "-f", "--unit=custom.service", "-o", "cat"]
    );
}

#[test]
fn probe_args_read_one_user_unit_entry_without_pager() {
    assert_eq!(
        probe_args("custom.service"),
        vec![
            "--user",
            "--no-pager",
            "--lines=1",
            "--unit=custom.service",
            "-o",
            "cat"
        ]
    );
}

#[test]
fn recent_journal_args_request_only_the_given_window() {
    assert_eq!(
        recent_args("unixnotis-daemon.service", 30),
        vec![
            "--user",
            "--no-pager",
            "--lines=30",
            "--unit=unixnotis-daemon.service",
            "-o",
            "cat"
        ]
    );
}

#[test]
fn daemon_unit_validation_rejects_option_like_whitespace_and_oversized_values() {
    for unit in [
        "",
        "--system.service",
        "contains space.service",
        "contains\ncontrol.service",
        "wrong.target",
        "two@@instances.service",
    ] {
        assert!(validate_daemon_unit(unit).is_err(), "{unit:?} must fail");
    }
    assert!(validate_daemon_unit(&format!("{}.service", "a".repeat(248))).is_err());
}

#[test]
fn daemon_unit_validation_accepts_plain_and_instantiated_services() {
    for unit in [
        "unixnotis-daemon.service",
        "unixnotis-daemon@desktop.service",
        "unixnotis\\x2ddaemon.service",
    ] {
        validate_daemon_unit(unit).expect("valid systemd service unit");
    }
}
