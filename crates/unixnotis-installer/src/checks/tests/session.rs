use std::env;

use crate::checks::{CheckItem, CheckState, Checks};
use crate::model::ActionMode;

use super::system::{hyprland_check, wayland_check};

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    // Session checks read process env directly, so tests use the crate-wide guard
    crate::tests::env::test_env_lock()
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

#[test]
fn ready_for_trial_requires_wayland_cargo_and_layer_shell_only() {
    let mut checks = passing_checks();
    checks.install_paths = item("Install paths", CheckState::Fail);
    checks.service_manager = item("Service manager", CheckState::Fail);

    // Trial mode runs workspace binaries directly, so install paths and
    // service-manager readiness should not block a local trial launch
    assert!(checks.ready_for(ActionMode::Test).is_ok());

    checks.wayland = item("Wayland", CheckState::Fail);
    assert_eq!(
        checks.ready_for(ActionMode::Test),
        Err("Wayland session required".to_string())
    );

    checks = passing_checks();
    checks.cargo = item("cargo", CheckState::Fail);
    assert_eq!(
        checks.ready_for(ActionMode::Test),
        Err("cargo is required for trial mode".to_string())
    );

    checks = passing_checks();
    checks.gtk4_layer_shell = item("gtk4-layer-shell", CheckState::Fail);
    assert_eq!(
        checks.ready_for(ActionMode::Test),
        Err("gtk4-layer-shell is required; is the gtk4-layer-shell package installed?".to_string())
    );
}

#[test]
fn ready_for_install_requires_runtime_service_manager_and_writable_paths() {
    let mut checks = passing_checks();
    checks.wayland = item("Wayland", CheckState::Fail);
    assert_eq!(
        checks.ready_for(ActionMode::Install),
        Err("Wayland session required".to_string())
    );

    checks = passing_checks();
    checks.service_manager = item("Service manager", CheckState::Fail);
    assert_eq!(
        checks.ready_for(ActionMode::Install),
        Err("supported service manager session required".to_string())
    );

    checks = passing_checks();
    checks.cargo = item("cargo", CheckState::Fail);
    assert_eq!(
        checks.ready_for(ActionMode::Install),
        Err("cargo is required for installation".to_string())
    );

    checks = passing_checks();
    checks.gtk4_layer_shell = item("gtk4-layer-shell", CheckState::Fail);
    assert_eq!(
        checks.ready_for(ActionMode::Install),
        Err("gtk4-layer-shell is required; is the gtk4-layer-shell package installed?".to_string())
    );

    checks = passing_checks();
    checks.install_paths = item("Install paths", CheckState::Fail);
    assert_eq!(
        checks.ready_for(ActionMode::Install),
        Err("install paths are not writable".to_string())
    );
}

#[test]
fn ready_for_uninstall_only_requires_backend_and_writable_paths() {
    let mut checks = passing_checks();
    checks.wayland = item("Wayland", CheckState::Fail);
    checks.cargo = item("cargo", CheckState::Fail);
    checks.gtk4_layer_shell = item("gtk4-layer-shell", CheckState::Fail);

    // Uninstall should remain available even when runtime launch checks fail
    assert!(checks.ready_for(ActionMode::Uninstall).is_ok());

    checks.service_manager = item("Service manager", CheckState::Fail);
    assert_eq!(
        checks.ready_for(ActionMode::Uninstall),
        Err("supported service manager session required".to_string())
    );

    checks = passing_checks();
    checks.install_paths = item("Install paths", CheckState::Fail);
    assert_eq!(
        checks.ready_for(ActionMode::Uninstall),
        Err("install paths are not writable".to_string())
    );
}

#[test]
fn ready_for_reset_never_blocks_on_environment_checks() {
    let checks = Checks {
        wayland: item("Wayland", CheckState::Fail),
        hyprland: item("Hyprland", CheckState::Fail),
        service_manager: item("Service manager", CheckState::Fail),
        cargo: item("cargo", CheckState::Fail),
        pkg_config: item("pkg-config", CheckState::Fail),
        gtk4_css_features: item("GTK4 CSS features", CheckState::Fail),
        gtk4_layer_shell: item("gtk4-layer-shell", CheckState::Fail),
        busctl: item("busctl", CheckState::Fail),
        dbus_update_env: item("dbus-update-activation-environment", CheckState::Fail),
        install_paths: item("Install paths", CheckState::Fail),
        path_contains_bin: item("Shell PATH", CheckState::Fail),
    };

    // Reset config can work from backups/defaults without Wayland or service
    // manager state, so all check failures should still allow this menu path
    assert!(checks.ready_for(ActionMode::Reset).is_ok());
}

fn passing_checks() -> Checks {
    Checks {
        wayland: item("Wayland", CheckState::Ok),
        hyprland: item("Hyprland", CheckState::Warn),
        service_manager: item("Service manager", CheckState::Ok),
        cargo: item("cargo", CheckState::Ok),
        pkg_config: item("pkg-config", CheckState::Ok),
        gtk4_css_features: item("GTK4 CSS features", CheckState::Ok),
        gtk4_layer_shell: item("gtk4-layer-shell", CheckState::Ok),
        busctl: item("busctl", CheckState::Ok),
        dbus_update_env: item("dbus-update-activation-environment", CheckState::Ok),
        install_paths: item("Install paths", CheckState::Ok),
        path_contains_bin: item("Shell PATH", CheckState::Warn),
    }
}

fn item(label: &'static str, state: CheckState) -> CheckItem {
    CheckItem {
        label,
        state,
        detail: format!("{state:?} test detail"),
    }
}
