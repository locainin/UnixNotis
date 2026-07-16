use std::os::unix::net::UnixListener;
use std::sync::{Mutex, MutexGuard, OnceLock};

use super::super::environment::*;
use crate::doctor::report::DoctorSeverity;

struct EnvGuard {
    name: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn set(name: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
        let previous = std::env::var_os(name);
        std::env::set_var(name, value);
        Self { name, previous }
    }

    fn remove(name: &'static str) -> Self {
        let previous = std::env::var_os(name);
        std::env::remove_var(name);
        Self { name, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(value) => std::env::set_var(self.name, value),
            None => std::env::remove_var(self.name),
        }
    }
}

fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("doctor environment lock")
}

#[test]
fn healthy_session_environment_reports_transport_without_exposing_address() {
    let _lock = env_lock();
    let root = std::env::temp_dir().join(format!(
        "unixnotis-doctor-session-environment-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create runtime directory");
    let socket_path = root.join("bus");
    let _listener = UnixListener::bind(&socket_path).expect("bind runtime bus socket");
    let _runtime = EnvGuard::set("XDG_RUNTIME_DIR", &root);
    let _address = EnvGuard::set(
        "DBUS_SESSION_BUS_ADDRESS",
        format!("unix:path={}", socket_path.display()),
    );
    let _wayland = EnvGuard::set("WAYLAND_DISPLAY", "wayland-1");

    let check = inspect_session_environment(true);

    assert_eq!(check.severity, DoctorSeverity::Pass);
    assert_eq!(check.data["dbus_transport"], "unix");
    assert!(!check
        .details
        .as_deref()
        .is_some_and(|details| details.contains(&socket_path.display().to_string())));
    std::fs::remove_dir_all(root).expect("remove runtime directory");
}

#[test]
fn missing_environment_becomes_actionable_when_bus_connection_failed() {
    let _lock = env_lock();
    let _runtime = EnvGuard::remove("XDG_RUNTIME_DIR");
    let _address = EnvGuard::remove("DBUS_SESSION_BUS_ADDRESS");
    let _wayland = EnvGuard::remove("WAYLAND_DISPLAY");
    let _session = EnvGuard::set("XDG_SESSION_TYPE", "tty");

    let check = inspect_session_environment(false);

    assert_eq!(check.severity, DoctorSeverity::Warning);
    assert!(check
        .hint
        .as_deref()
        .is_some_and(|hint| hint.contains("XDG_RUNTIME_DIR is not set")));
}
