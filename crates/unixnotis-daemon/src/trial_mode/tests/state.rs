use anyhow::anyhow;

use super::{prepare_trial, restore_after_prepare_failure, RestoreAction, TrialState};
use crate::cli::{Args, RestoreStrategy};

impl TrialState {
    pub(crate) const fn with_restore_action_for_test(action: RestoreAction) -> Self {
        Self {
            restore_action: Some(action),
        }
    }
}

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

#[tokio::test]
async fn trial_preparation_propagates_broker_failure_instead_of_assuming_unowned() {
    let connection = zbus::Connection::session()
        .await
        .expect("session bus connection");
    let proxy = zbus::fdo::DBusProxy::new(&connection)
        .await
        .expect("D-Bus proxy");
    connection.close().await.expect("close test bus connection");
    let notifications = zbus::names::BusName::try_from(unixnotis_core::NOTIFICATIONS_BUS_NAME)
        .expect("Notifications bus name");
    let args = Args {
        config: None,
        trial: true,
        restore: RestoreStrategy::Auto,
        yes: true,
        restore_wait_ms: 1,
        check: false,
        run_seconds: None,
    };

    assert!(prepare_trial(&args, &proxy, notifications).await.is_err());
}
