use clap::Parser;

use super::{
    combine_run_and_restore, restore_previous_or_fail,
    trial_mode::{RestoreAction, TrialState},
    Args, RestoreStrategy,
};
use anyhow::anyhow;

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
fn startup_and_restoration_errors_are_both_preserved() {
    let error = combine_run_and_restore(
        Err(anyhow!("object registration failed")),
        Err(anyhow!("previous daemon restart failed")),
    )
    .expect_err("combined failure");
    let message = format!("{error:#}");

    assert!(message.contains("object registration failed"));
    assert!(message.contains("trial restoration also failed"));
    assert!(message.contains("previous daemon restart failed"));
}

#[test]
fn restoration_error_is_returned_after_successful_runtime() {
    let error = combine_run_and_restore(Ok(()), Err(anyhow!("restore failed")))
        .expect_err("restore failure");

    assert_eq!(error.to_string(), "restore failed");
}

#[test]
fn trial_restore_action_can_be_consumed_exactly_once() {
    let mut trial = TrialState::with_restore_action_for_test(RestoreAction::Systemd {
        unit: "mako.service".to_string(),
    });

    assert!(trial.take_restore_action().is_some());
    assert!(trial.take_restore_action().is_none());
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
