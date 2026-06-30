use std::time::Duration;

use crate::test_support::{env_lock, EnvVarGuard, TempRoot};
use crate::{Args, RestoreStrategy};

use super::*;

fn default_args() -> Args {
    Args {
        config: None,
        trial: false,
        restore: RestoreStrategy::Auto,
        yes: false,
        restore_wait_ms: 2_000,
        check: false,
        run_seconds: None,
    }
}

#[cfg(unix)]
fn bind_wayland_socket(path: &std::path::Path) -> std::os::unix::net::UnixListener {
    // Holding the listener keeps the filesystem entry as a real socket for metadata checks
    std::os::unix::net::UnixListener::bind(path).expect("bind wayland socket")
}

#[test]
fn load_config_rejects_missing_custom_config_path() {
    let root = TempRoot::new("runtime-config-missing");
    let mut args = default_args();
    args.config = Some(root.join("missing.toml"));

    assert!(load_config(&args).is_err());
}

#[test]
fn init_tracing_installs_a_global_dispatcher() {
    let _guard = env_lock();
    let _rust_log = EnvVarGuard::set("RUST_LOG", "warn");
    let config = unixnotis_core::Config::default();

    init_tracing(&config);

    assert!(tracing::dispatcher::has_been_set());
}

#[test]
fn choose_wayland_fallback_prefers_wayland_zero() {
    let chosen = choose_wayland_fallback(vec![
        "wayland-2".to_string(),
        "wayland-0".to_string(),
        "wayland-1".to_string(),
    ]);
    assert_eq!(chosen.as_deref(), Some("wayland-0"));
}

#[test]
fn choose_wayland_fallback_sorts_nonzero_candidates() {
    let chosen = choose_wayland_fallback(vec![
        "wayland-7".to_string(),
        "wayland-3".to_string(),
        "wayland-5".to_string(),
    ]);
    assert_eq!(chosen.as_deref(), Some("wayland-3"));
}

#[test]
fn choose_wayland_fallback_returns_none_for_empty_list() {
    assert!(choose_wayland_fallback(Vec::new()).is_none());
}

#[cfg(unix)]
#[test]
fn detect_wayland_display_uses_existing_env_socket() {
    let _guard = env_lock();
    let root = TempRoot::new("runtime-wayland-env");
    let socket = root.join("wayland-7");
    let _listener = bind_wayland_socket(&socket);
    let _runtime = EnvVarGuard::set("XDG_RUNTIME_DIR", root.path());
    let _display = EnvVarGuard::set("WAYLAND_DISPLAY", "wayland-7");

    assert_eq!(detect_wayland_display().as_deref(), Some("wayland-7"));
}

#[cfg(unix)]
#[test]
fn wayland_socket_exists_returns_true_only_for_real_socket() {
    let _guard = env_lock();
    let root = TempRoot::new("runtime-wayland-socket-probe");
    let socket = root.join("wayland-9");
    let _listener = bind_wayland_socket(&socket);
    std::fs::write(root.join("wayland-file"), b"not a socket").expect("write decoy");
    let _runtime = EnvVarGuard::set("XDG_RUNTIME_DIR", root.path());

    assert!(wayland_socket_exists("wayland-9"));
    assert!(!wayland_socket_exists("wayland-file"));
    assert!(!wayland_socket_exists("missing"));
}

#[cfg(unix)]
#[test]
fn detect_wayland_display_rejects_regular_file_from_env() {
    let _guard = env_lock();
    let root = TempRoot::new("runtime-wayland-regular-env");
    std::fs::write(root.join("wayland-7"), b"not a socket").expect("write decoy");
    let _runtime = EnvVarGuard::set("XDG_RUNTIME_DIR", root.path());
    let _display = EnvVarGuard::set("WAYLAND_DISPLAY", "wayland-7");

    assert!(detect_wayland_display().is_none());
}

#[cfg(unix)]
#[test]
fn detect_wayland_display_scans_runtime_sockets_and_ignores_regular_files() {
    let _guard = env_lock();
    let root = TempRoot::new("runtime-wayland-scan");
    std::fs::write(root.join("wayland-0"), b"not a socket").expect("write decoy");
    let _listener = bind_wayland_socket(&root.join("wayland-3"));
    let _runtime = EnvVarGuard::set("XDG_RUNTIME_DIR", root.path());
    let _display = EnvVarGuard::remove("WAYLAND_DISPLAY");

    assert_eq!(detect_wayland_display().as_deref(), Some("wayland-3"));
}

#[test]
fn apply_wayland_env_sets_display_and_default_session_type() {
    let _guard = env_lock();
    let _display = EnvVarGuard::remove("WAYLAND_DISPLAY");
    let _session = EnvVarGuard::remove("XDG_SESSION_TYPE");

    apply_wayland_env("wayland-test");

    assert_eq!(
        std::env::var("WAYLAND_DISPLAY").as_deref(),
        Ok("wayland-test")
    );
    assert_eq!(std::env::var("XDG_SESSION_TYPE").as_deref(), Ok("wayland"));
}

#[test]
fn apply_wayland_env_preserves_existing_session_type() {
    let _guard = env_lock();
    let _display = EnvVarGuard::remove("WAYLAND_DISPLAY");
    let _session = EnvVarGuard::set("XDG_SESSION_TYPE", "wayland-custom");

    apply_wayland_env("wayland-test");

    assert_eq!(
        std::env::var("WAYLAND_DISPLAY").as_deref(),
        Ok("wayland-test")
    );
    assert_eq!(
        std::env::var("XDG_SESSION_TYPE").as_deref(),
        Ok("wayland-custom")
    );
}

#[cfg(unix)]
#[tokio::test]
#[allow(
    clippy::await_holding_lock,
    reason = "Wayland env tests must keep process-global env stable while async socket polling runs"
)]
async fn ensure_wayland_session_detects_socket_and_applies_env() {
    let _guard = env_lock();
    let root = TempRoot::new("runtime-wayland-ensure");
    let _listener = bind_wayland_socket(&root.join("wayland-5"));
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
#[allow(
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
#[allow(
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
#[allow(
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
        bind_wayland_socket(&task_socket)
    });

    let display = wait_for_wayland_display(Duration::from_millis(500))
        .await
        .expect("late socket should be detected before timeout");
    assert_eq!(display, "wayland-late");

    let listener = late_socket.await.expect("late socket task");
    drop(listener);
}
