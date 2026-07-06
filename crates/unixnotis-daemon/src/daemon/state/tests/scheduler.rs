use std::time::Duration;

use crate::expire::{ExpirationCommand, ExpirationScheduler};
use crate::test_support::daemon_state_for_test;

#[tokio::test]
async fn cancel_expiration_sends_cancel_command_when_scheduler_is_installed() {
    let state = daemon_state_for_test(false).await;
    let (scheduler, mut receiver) = ExpirationScheduler::channel_for_test();

    state.set_scheduler(scheduler);
    state.cancel_expiration(42);

    let command = tokio::time::timeout(Duration::from_millis(100), receiver.recv())
        .await
        .expect("cancel command should arrive")
        .expect("scheduler channel should stay open");
    match command {
        ExpirationCommand::Cancel { id } => assert_eq!(id, 42),
        ExpirationCommand::Schedule { .. } => panic!("cancel should not schedule a deadline"),
    }
}

#[tokio::test]
async fn cancel_expirations_sends_cancel_for_each_id_in_order() {
    let state = daemon_state_for_test(false).await;
    let (scheduler, mut receiver) = ExpirationScheduler::channel_for_test();

    state.set_scheduler(scheduler);
    state.cancel_expirations(&[7, 8, 9]);

    let mut ids = Vec::new();
    for _ in 0..3 {
        let command = tokio::time::timeout(Duration::from_millis(100), receiver.recv())
            .await
            .expect("cancel command should arrive")
            .expect("scheduler channel should stay open");
        match command {
            ExpirationCommand::Cancel { id } => ids.push(id),
            ExpirationCommand::Schedule { .. } => panic!("cancel should not schedule a deadline"),
        }
    }

    assert_eq!(ids, [7, 8, 9]);
    assert!(receiver.try_recv().is_err());
}

#[tokio::test]
async fn duplicate_scheduler_install_keeps_original_sender() {
    let state = daemon_state_for_test(false).await;
    let (first_scheduler, mut first_receiver) = ExpirationScheduler::channel_for_test();
    let (second_scheduler, mut second_receiver) = ExpirationScheduler::channel_for_test();

    state.set_scheduler(first_scheduler);
    state.set_scheduler(second_scheduler);
    state.cancel_expiration(11);

    let command = tokio::time::timeout(Duration::from_millis(100), first_receiver.recv())
        .await
        .expect("original scheduler should receive cancel")
        .expect("original scheduler channel should stay open");
    match command {
        ExpirationCommand::Cancel { id } => assert_eq!(id, 11),
        ExpirationCommand::Schedule { .. } => panic!("cancel should not schedule a deadline"),
    }
    assert!(second_receiver.try_recv().is_err());
}

#[tokio::test]
async fn missing_scheduler_cancel_is_a_noop() {
    let state = daemon_state_for_test(false).await;

    state.cancel_expiration(1);
    state.cancel_expirations(&[2, 3]);

    assert!(!state.mark_missing_scheduler_warning_needed());
}

#[tokio::test]
async fn missing_scheduler_warning_guard_reports_only_first_missing_scheduler_use() {
    let state = daemon_state_for_test(false).await;

    assert!(state.mark_missing_scheduler_warning_needed());
    assert!(!state.mark_missing_scheduler_warning_needed());
}
