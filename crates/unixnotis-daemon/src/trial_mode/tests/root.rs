use anyhow::anyhow;

use super::{restore_after_prepare_failure, RestoreAction, TrialState};

#[test]
fn preparation_failure_restores_once_and_preserves_both_errors() {
    let mut trial = TrialState::with_restore_action_for_test(RestoreAction::Command {
        program: "/definitely/missing/unixnotis-prepare-restore".to_string(),
        args: Vec::new(),
    });

    let error =
        restore_after_prepare_failure(&mut trial, anyhow!("notification owner release timed out"));
    let message = format!("{error:#}");

    assert!(message.contains("notification owner release timed out"));
    assert!(message.contains("trial restoration also failed"));
    assert!(trial.take_restore_action().is_none());
}
