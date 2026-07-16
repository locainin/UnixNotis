use std::path::Path;
use std::sync::{Mutex, MutexGuard, OnceLock};

use super::*;

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
        .expect("doctor config environment lock")
}

#[test]
fn home_paths_are_redacted_without_hiding_the_relative_location() {
    let home = std::env::var("HOME").expect("HOME");
    let path = Path::new(&home).join(".config/unixnotis/config.toml");

    assert_eq!(redact_home(&path), "$HOME/.config/unixnotis/config.toml");
}

#[test]
fn paths_outside_home_remain_visible() {
    assert_eq!(
        redact_home(Path::new("/tmp/unixnotis.toml")),
        "/tmp/unixnotis.toml"
    );
}

#[test]
fn free_form_output_replaces_the_literal_home_prefix() {
    let home = std::env::var("HOME").expect("HOME");
    let raw = format!("FragmentPath={home}/.config/systemd/user/unixnotis-daemon.service");

    assert_eq!(
        redact_home_text(&raw),
        "FragmentPath=$HOME/.config/systemd/user/unixnotis-daemon.service"
    );
}

#[test]
fn shareable_parse_errors_never_echo_original_config_text() {
    let error = unixnotis_core::ConfigError::ParseFailed(
        "secret_command = '/home/private/tool'".to_string(),
    );

    let detail = error.shareable_summary();

    assert_eq!(detail, "Configuration TOML or schema is invalid");
    assert!(!detail.contains("secret_command"));
    assert!(!detail.contains("/home/private"));
}

#[test]
fn explicit_config_detection_distinguishes_missing_empty_and_present_values() {
    let _lock = env_lock();
    let missing_guard = EnvGuard::remove(CONFIG_PATH_ENV);
    assert!(!explicit_config_path_is_set());
    drop(missing_guard);

    let empty_guard = EnvGuard::set(CONFIG_PATH_ENV, "");
    assert!(!explicit_config_path_is_set());
    drop(empty_guard);

    let _present = EnvGuard::set(CONFIG_PATH_ENV, "/tmp/unixnotis-doctor-config.toml");
    assert!(explicit_config_path_is_set());
}

#[test]
fn config_override_drives_resolution_and_preserves_diagnostic_details() {
    let _lock = env_lock();
    let root = std::env::temp_dir().join(format!(
        "unixnotis-doctor-config-override-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create doctor config directory");
    let config_path = root.join("config.toml");
    std::fs::write(
        &config_path,
        format!("config_version = {CURRENT_CONFIG_VERSION}\n[panel]\nunknown_doctor_key = true\n"),
    )
    .expect("write doctor config");
    let _config = EnvGuard::set(CONFIG_PATH_ENV, &config_path);

    assert_eq!(
        resolve_config_path().expect("resolve overridden config path"),
        config_path
    );
    let result = inspect_config();

    assert_eq!(result.config_path, config_path);
    let diagnostic = result
        .checks
        .iter()
        .find(|check| check.id.contains("config.unknown-key"))
        .expect("unknown key diagnostic");
    assert!(diagnostic
        .details
        .as_deref()
        .is_some_and(|details| details.contains("Key: panel.unknown_doctor_key")));
    std::fs::remove_dir_all(root).expect("remove doctor config directory");
}

#[test]
fn explicit_missing_config_is_an_error_instead_of_a_default_request() {
    let _lock = env_lock();
    let path = std::env::temp_dir().join(format!(
        "unixnotis-doctor-missing-config-{}/config.toml",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let _config = EnvGuard::set(CONFIG_PATH_ENV, &path);

    let result = inspect_config();

    assert!(result.report.is_none());
    assert!(result.checks.iter().any(|check| {
        check.id == "config.acceptance"
            && check.severity == DoctorSeverity::Error
            && check.summary == "Explicit configuration file does not exist"
    }));
}
