#[cfg(unix)]
use crate::test_support::{env_lock, EnvVarGuard, TempRoot};

#[cfg(unix)]
use super::{detect_wayland_display, test_support, wayland_socket_exists};

#[cfg(unix)]
#[test]
fn detect_wayland_display_uses_existing_env_socket() {
    let _guard = env_lock();
    let root = TempRoot::new("runtime-wayland-env");
    let socket = root.join("wayland-7");
    let _listener = test_support::bind_wayland_socket(&socket);
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
    let _listener = test_support::bind_wayland_socket(&socket);
    std::fs::write(root.join("wayland-file"), b"not a socket").expect("write decoy");
    let _runtime = EnvVarGuard::set("XDG_RUNTIME_DIR", root.path());

    // Only socket entries should satisfy the Wayland display probe
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

    // WAYLAND_DISPLAY alone is not enough; the referenced path must be a socket
    assert!(detect_wayland_display().is_none());
}

#[cfg(unix)]
#[test]
fn detect_wayland_display_scans_runtime_sockets_and_ignores_regular_files() {
    let _guard = env_lock();
    let root = TempRoot::new("runtime-wayland-scan");
    std::fs::write(root.join("wayland-0"), b"not a socket").expect("write decoy");
    let _listener = test_support::bind_wayland_socket(&root.join("wayland-3"));
    let _runtime = EnvVarGuard::set("XDG_RUNTIME_DIR", root.path());
    let _display = EnvVarGuard::remove("WAYLAND_DISPLAY");

    assert_eq!(detect_wayland_display().as_deref(), Some("wayland-3"));
}
