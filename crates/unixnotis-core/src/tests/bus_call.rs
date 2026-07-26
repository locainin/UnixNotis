use std::time::Duration;

use super::{timed_dbus_call, timed_dbus_call_with_timeout};

#[tokio::test]
async fn internal_dbus_call_timeout_is_hard_and_bounded() {
    let call = std::future::pending::<zbus::Result<()>>();
    let error = timed_dbus_call_with_timeout(Duration::from_millis(1), call)
        .await
        .expect_err("pending method must time out");

    assert!(error.to_string().contains("timed out"));
}

#[tokio::test]
async fn internal_dbus_call_returns_success_without_delay() {
    let value = timed_dbus_call(std::future::ready(Ok::<_, zbus::Error>(42)))
        .await
        .expect("ready call should pass");

    assert_eq!(value, 42);
}
