use anyhow::anyhow;

use crate::runtime::trial_cleanup::{combine_run_and_restore, restore_previous_or_fail};
use crate::trial_mode::{RestoreAction, TrialState};

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
fn restore_previous_or_fail_returns_spawn_error() {
    let action = RestoreAction::Command {
        program: "/definitely/missing/unixnotis-restore-target".to_string(),
        args: Vec::new(),
    };

    let error = restore_previous_or_fail(action).expect_err("restore failure must fail trial mode");

    assert!(error
        .to_string()
        .contains("restore previous notification daemon"));
}
