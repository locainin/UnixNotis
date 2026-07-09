use clap::Parser;

use super::{restore_previous_or_fail, trial_mode::RestoreAction, Args, RestoreStrategy};

#[test]
fn args_parse_check_mode_without_trial_flags() {
    let args = Args::try_parse_from(["unixnotis-daemon", "--check"]).expect("parse args");

    assert!(args.check);
    assert!(!args.trial);
    assert!(matches!(args.restore, RestoreStrategy::Auto));
    assert_eq!(args.restore_wait_ms, 2_000);
}

#[test]
fn args_parse_trial_restore_process_and_run_seconds() {
    let args = Args::try_parse_from([
        "unixnotis-daemon",
        "--trial",
        "--yes",
        "--restore",
        "process",
        "--restore-wait-ms",
        "125",
        "--run-seconds",
        "9",
    ])
    .expect("parse trial args");

    // Trial flags are safety controls, so the parsed values need direct coverage
    assert!(args.trial);
    assert!(args.yes);
    assert!(matches!(args.restore, RestoreStrategy::Process));
    assert_eq!(args.restore_wait_ms, 125);
    assert_eq!(args.run_seconds, Some(9));
}

#[test]
fn restore_previous_or_fail_returns_error_when_restore_command_cannot_spawn() {
    let action = RestoreAction::Command {
        program: "/definitely/missing/unixnotis-restore-target".to_string(),
        args: Vec::new(),
    };

    let err = restore_previous_or_fail(action).expect_err("restore failure must fail trial mode");

    // Trial mode must not report success when the old daemon could not be restarted
    assert!(err
        .to_string()
        .contains("restore previous notification daemon"));
}
