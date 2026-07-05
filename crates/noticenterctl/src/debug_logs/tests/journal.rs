use std::env;

use super::super::journal::{daemon_unit_from_env, follow_args, probe_args};

#[test]
fn daemon_unit_from_env_uses_override_when_present() {
    let unit = daemon_unit_from_env(|key| {
        assert_eq!(key, "UNIXNOTIS_DAEMON_UNIT");
        Ok("custom.service".to_string())
    });

    assert_eq!(unit, "custom.service");
}

#[test]
fn daemon_unit_from_env_uses_default_when_override_is_absent() {
    let unit = daemon_unit_from_env(|_| Err(env::VarError::NotPresent));

    assert_eq!(unit, "unixnotis-daemon.service");
}

#[test]
fn follow_args_target_user_unit_and_stream_plain_messages() {
    assert_eq!(
        follow_args("custom.service"),
        vec!["--user", "-f", "-u", "custom.service", "-o", "cat"]
    );
}

#[test]
fn probe_args_read_one_user_unit_entry_without_pager() {
    assert_eq!(
        probe_args("custom.service"),
        vec![
            "--user",
            "--no-pager",
            "-n",
            "1",
            "-u",
            "custom.service",
            "-o",
            "cat"
        ]
    );
}
