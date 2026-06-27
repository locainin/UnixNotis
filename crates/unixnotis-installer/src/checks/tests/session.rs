use std::env;
use std::sync::{Mutex, OnceLock};

use crate::checks::CheckState;

use super::system::{hyprland_check, wayland_check};

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    // Session checks read process env directly, so tests serialize env mutation
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("env lock")
}

struct EnvGuard {
    key: &'static str,
    old: Option<String>,
}

impl EnvGuard {
    fn set(key: &'static str, value: Option<&str>) -> Self {
        let old = env::var(key).ok();
        match value {
            // Use the real process env path because production checks read it directly
            Some(value) => env::set_var(key, value),
            None => env::remove_var(key),
        }
        Self { key, old }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.old {
            // Restore test mutations so later environment-sensitive checks stay isolated
            Some(value) => env::set_var(self.key, value),
            None => env::remove_var(self.key),
        }
    }
}

#[test]
fn wayland_check_fails_when_runtime_dir_is_missing() {
    let _lock = env_lock();
    let _session = EnvGuard::set("XDG_SESSION_TYPE", Some("wayland"));
    let _display = EnvGuard::set("WAYLAND_DISPLAY", Some("wayland-test"));
    let _runtime = EnvGuard::set("XDG_RUNTIME_DIR", None);

    let item = wayland_check();

    assert_eq!(item.state, CheckState::Fail);
    assert_eq!(item.detail, "session missing XDG_RUNTIME_DIR");
}

#[test]
fn wayland_check_accepts_wayland_display_with_runtime_dir() {
    let _lock = env_lock();
    let _session = EnvGuard::set("XDG_SESSION_TYPE", None);
    let _display = EnvGuard::set("WAYLAND_DISPLAY", Some("wayland-test"));
    let _runtime = EnvGuard::set("XDG_RUNTIME_DIR", Some("/run/user/1000"));

    let item = wayland_check();

    assert_eq!(item.state, CheckState::Ok);
    assert_eq!(item.detail, "session detected");
}

#[test]
fn hyprland_check_warns_when_instance_signature_is_missing() {
    let _lock = env_lock();
    let _signature = EnvGuard::set("HYPRLAND_INSTANCE_SIGNATURE", None);

    let item = hyprland_check();

    assert_eq!(item.state, CheckState::Warn);
    assert_eq!(item.detail, "not detected");
}

#[test]
fn hyprland_check_accepts_instance_signature() {
    let _lock = env_lock();
    let _signature = EnvGuard::set("HYPRLAND_INSTANCE_SIGNATURE", Some("test-signature"));

    let item = hyprland_check();

    assert_eq!(item.state, CheckState::Ok);
    assert_eq!(item.detail, "instance detected");
}
