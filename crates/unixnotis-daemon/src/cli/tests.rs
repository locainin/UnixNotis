use clap::Parser;

use super::{Args, RestoreStrategy};

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

    assert!(args.trial);
    assert!(args.yes);
    assert!(matches!(args.restore, RestoreStrategy::Process));
    assert_eq!(args.restore_wait_ms, 125);
    assert_eq!(args.run_seconds, Some(9));
}
