use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use super::super::config::*;
use crate::doctor::report::DoctorSeverity;
use crate::doctor::report::{redact_home, redact_home_text};
use unixnotis_core::util::CONFIG_PATH_ENV;
use unixnotis_core::CURRENT_CONFIG_VERSION;

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
        "secret_command = 'private-parser-sentinel'".to_string(),
    );

    let detail = error.shareable_summary();

    assert_eq!(detail, "Configuration TOML or schema is invalid");
    assert!(!detail.contains("secret_command"));
    assert!(!detail.contains("private-parser-sentinel"));
}

#[test]
fn config_resolution_reports_default_environment_and_cli_sources() {
    let _lock = env_lock();
    let missing_guard = EnvGuard::remove(CONFIG_PATH_ENV);
    assert_eq!(
        resolve_config_path(None)
            .expect("resolve default config path")
            .1,
        ConfigPathSource::Default
    );
    drop(missing_guard);

    let empty_guard = EnvGuard::set(CONFIG_PATH_ENV, "");
    assert_eq!(
        resolve_config_path(None).expect("resolve empty override").1,
        ConfigPathSource::Default
    );
    drop(empty_guard);

    let present = EnvGuard::set(CONFIG_PATH_ENV, "/tmp/unixnotis-doctor-config.toml");
    assert_eq!(
        resolve_config_path(None)
            .expect("resolve environment override")
            .1,
        ConfigPathSource::Environment
    );
    drop(present);
    assert_eq!(
        resolve_config_path(Some(PathBuf::from("/tmp/doctor-cli.toml")))
            .expect("resolve CLI override")
            .1,
        ConfigPathSource::Cli
    );
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
        resolve_config_path(None).expect("resolve overridden config path"),
        (config_path.clone(), ConfigPathSource::Environment)
    );
    let result = inspect_config(None);

    assert_eq!(result.config_path, config_path);
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "config.unknown-key")
        .expect("unknown key diagnostic");
    assert!(diagnostic
        .path
        .as_deref()
        .is_some_and(|path| path == "panel.unknown_doctor_key"));
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

    let result = inspect_config(None);

    assert!(result.report.is_none());
    assert!(result.checks.iter().any(|check| {
        check.id == "config.acceptance"
            && check.severity == DoctorSeverity::Error
            && check.summary == "Explicit configuration file does not exist"
    }));
}

#[test]
fn cli_config_path_outranks_the_environment_override() {
    let _lock = env_lock();
    let _environment = EnvGuard::set(CONFIG_PATH_ENV, "/tmp/environment-config.toml");
    let cli = PathBuf::from("/tmp/cli-config.toml");

    let resolved = resolve_config_path(Some(cli.clone())).expect("resolve CLI config path");

    assert_eq!(resolved, (cli, ConfigPathSource::Cli));
}
