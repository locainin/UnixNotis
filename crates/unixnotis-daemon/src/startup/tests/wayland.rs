use std::time::Duration;

use crate::test_support::{env_lock, EnvVarGuard, TempRoot};

use crate::startup::tests::support as test_support;
use crate::startup::wayland::{ensure_wayland_session, wait_for_wayland_display};

#[cfg(unix)]
#[tokio::test]
#[expect(
    clippy::await_holding_lock,
    reason = "Wayland env tests must keep process-global env stable while async socket polling runs"
)]
async fn ensure_wayland_session_detects_socket_and_applies_env() {
    let _guard = env_lock();
    let root = TempRoot::new("runtime-wayland-ensure");
    let _listener = test_support::bind_wayland_socket(&root.join("wayland-5"));
    let _runtime = EnvVarGuard::set("XDG_RUNTIME_DIR", root.path());
    let _display = EnvVarGuard::set("WAYLAND_DISPLAY", "wayland-5");
    let _session = EnvVarGuard::remove("XDG_SESSION_TYPE");

    ensure_wayland_session(Duration::from_millis(1))
        .await
        .expect("socket should satisfy wayland check");

    assert_eq!(std::env::var("WAYLAND_DISPLAY").as_deref(), Ok("wayland-5"));
    assert_eq!(std::env::var("XDG_SESSION_TYPE").as_deref(), Ok("wayland"));
}

#[tokio::test]
#[expect(
    clippy::await_holding_lock,
    reason = "Wayland env tests must keep process-global env stable while async socket polling runs"
)]
async fn ensure_wayland_session_rejects_non_wayland_session_without_socket() {
    let _guard = env_lock();
    let root = TempRoot::new("runtime-wayland-x11");
    let _runtime = EnvVarGuard::set("XDG_RUNTIME_DIR", root.path());
    let _display = EnvVarGuard::remove("WAYLAND_DISPLAY");
    let _session = EnvVarGuard::set("XDG_SESSION_TYPE", "x11");

    let err = ensure_wayland_session(Duration::from_millis(1))
        .await
        .expect_err("x11 without socket should fail");
    assert!(err.to_string().contains("XDG_SESSION_TYPE=x11"));
}

#[tokio::test]
#[expect(
    clippy::await_holding_lock,
    reason = "Wayland env tests must keep process-global env stable while async socket polling runs"
)]
async fn ensure_wayland_session_times_out_for_wayland_session_without_socket() {
    let _guard = env_lock();
    let root = TempRoot::new("runtime-wayland-timeout");
    let _runtime = EnvVarGuard::set("XDG_RUNTIME_DIR", root.path());
    let _display = EnvVarGuard::remove("WAYLAND_DISPLAY");
    let _session = EnvVarGuard::set("XDG_SESSION_TYPE", "wayland");

    let err = ensure_wayland_session(Duration::ZERO)
        .await
        .expect_err("missing socket should fail");
    assert!(err.to_string().contains("Wayland session not detected"));
}

#[cfg(unix)]
#[tokio::test]
#[expect(
    clippy::await_holding_lock,
    reason = "Wayland env tests must keep process-global env stable while async socket polling runs"
)]
async fn wait_for_wayland_display_observes_late_socket_before_timeout() {
    let _guard = env_lock();
    let root = TempRoot::new("runtime-wayland-late");
    let socket = root.join("wayland-late");
    let _runtime = EnvVarGuard::set("XDG_RUNTIME_DIR", root.path());
    let _display = EnvVarGuard::set("WAYLAND_DISPLAY", "wayland-late");

    let task_socket = socket.clone();
    let late_socket = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(20)).await;
        test_support::bind_wayland_socket(&task_socket)
    });

    let display = wait_for_wayland_display(Duration::from_millis(500))
        .await
        .expect("late socket should be detected before timeout");
    assert_eq!(display, "wayland-late");

    let listener = late_socket.await.expect("late socket task");
    drop(listener);
}
