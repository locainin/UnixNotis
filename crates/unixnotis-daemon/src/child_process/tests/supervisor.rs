use std::time::Duration;

use tokio::process::Command;
use tokio::sync::watch;
use tokio::time::sleep;

use super::{
    shutdown_is_terminal, terminate_child, wait_error_needs_recovery, wait_for_retry_or_shutdown,
};
use crate::child_process::{RestartBackoff, HEALTHY_RUNTIME_SECS, RESTART_BASE_MS, RESTART_MAX_MS};
use crate::test_support::TempRoot;

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

#[tokio::test]
async fn terminate_child_allows_a_slow_graceful_exit_before_escalating() {
    let root = TempRoot::new("child-term-ready");
    let ready_marker = root.join("ready");
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(
            "trap 'sleep 0.7; exit 0' TERM; : > \"$UNIXNOTIS_TEST_READY_MARKER\"; while :; do sleep 0.05; done",
        )
        .env("UNIXNOTIS_TEST_READY_MARKER", &ready_marker)
        .kill_on_drop(true)
        .spawn()
        .expect("spawn graceful child");
    tokio::time::timeout(Duration::from_secs(2), async {
        while !ready_marker.is_file() {
            // The marker is created only after the shell installs its TERM handler
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("child should report TERM readiness");

    terminate_child(&mut child, "graceful-test").await;

    let status = child
        .try_wait()
        .expect("poll graceful child")
        .expect("graceful child should exit");
    assert!(status.success(), "SIGTERM handler should exit successfully");
}
