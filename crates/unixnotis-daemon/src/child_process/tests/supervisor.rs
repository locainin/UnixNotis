use std::time::Duration;

use tokio::sync::watch;

use super::{shutdown_is_terminal, wait_error_needs_recovery, wait_for_retry_or_shutdown};
use crate::child_process::{RestartBackoff, HEALTHY_RUNTIME_SECS, RESTART_BASE_MS, RESTART_MAX_MS};

#[test]
fn crash_loop_backoff_starts_at_base() {
    let mut backoff = RestartBackoff::new();
    assert_eq!(
        backoff.next_delay(Duration::from_secs(0)),
        Duration::from_millis(RESTART_BASE_MS)
    );
}

#[test]
fn crash_loop_backoff_caps_at_max() {
    let mut backoff = RestartBackoff::new();
    for _ in 0..8 {
        let _ = backoff.next_delay(Duration::from_secs(0));
    }
    assert_eq!(backoff.current, Duration::from_millis(RESTART_MAX_MS));
}

#[test]
fn healthy_runtime_restarts_immediately() {
    let mut backoff = RestartBackoff::new();
    let _ = backoff.next_delay(Duration::from_secs(0));
    assert_eq!(
        backoff.next_delay(Duration::from_secs(HEALTHY_RUNTIME_SECS)),
        Duration::ZERO
    );
}

#[test]
fn wait_error_needs_recovery_when_child_state_is_unknown() {
    assert!(wait_error_needs_recovery(&Ok(false)));
    assert!(wait_error_needs_recovery(&Err(std::io::Error::other(
        "probe failed"
    ))));
    assert!(!wait_error_needs_recovery(&Ok(true)));
}

#[test]
fn shutdown_is_terminal_when_channel_is_closed() {
    let (tx, rx) = watch::channel(false);
    drop(tx);
    let mut rx = rx;
    assert!(shutdown_is_terminal(None, &mut rx));
}

#[test]
fn shutdown_is_terminal_when_flag_is_true() {
    let (_tx, rx) = watch::channel(true);
    let mut rx = rx;
    assert!(shutdown_is_terminal(None, &mut rx));
}

#[test]
fn shutdown_is_not_terminal_for_open_false_channel() {
    let (_tx, rx) = watch::channel(false);
    let mut rx = rx;
    assert!(!shutdown_is_terminal(None, &mut rx));
}

#[tokio::test]
async fn retry_wait_returns_immediately_for_zero_delay_when_shutdown_is_already_true() {
    let (_tx, rx) = watch::channel(true);
    let mut rx = rx;

    assert!(wait_for_retry_or_shutdown(Duration::ZERO, &mut rx).await);
}

#[tokio::test]
async fn retry_wait_sleeps_for_delay_when_shutdown_does_not_change() {
    let (_tx, rx) = watch::channel(false);
    let mut rx = rx;

    assert!(!wait_for_retry_or_shutdown(Duration::from_millis(1), &mut rx).await);
}

#[tokio::test]
async fn retry_wait_stops_when_shutdown_changes_before_delay_finishes() {
    let (tx, rx) = watch::channel(false);
    let mut rx = rx;

    let waiter =
        tokio::spawn(
            async move { wait_for_retry_or_shutdown(Duration::from_secs(5), &mut rx).await },
        );
    tx.send(true).expect("send shutdown");

    assert!(waiter.await.expect("waiter task"));
}
