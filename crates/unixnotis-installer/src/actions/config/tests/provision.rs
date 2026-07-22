//! End-to-end configuration provisioning tests

use std::fs;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};

use crate::actions::ActionContext;
use crate::app::events::UiMessage;
use crate::detect::Detection;
use crate::model::ActionMode;
use crate::paths::InstallPaths;
use crate::service_manager::ServiceManager;
use crate::test_support::env::{test_env_lock, EnvGuard};
use unixnotis_core::Config;

use super::super::provision::{ensure_config, reset_config};

fn test_paths(root: &std::path::Path) -> InstallPaths {
    InstallPaths {
        repo_root: root.join("repo"),
        bin_dir: root.join("home").join(".local").join("bin"),
        service: ServiceManager::systemd_user(root.join("service")),
    }
}

fn test_context<'a>(detection: &'a Detection, paths: &'a InstallPaths) -> ActionContext<'a> {
    let (log_tx, _log_rx) = mpsc::sync_channel::<UiMessage>(64);
    ActionContext {
        detection,
        paths,
        install_state: None,
        log_tx,
        action_mode: ActionMode::Install,
        restore_backup: None,
        service_reload_required: Arc::new(AtomicBool::new(false)),
    }
}

#[test]
fn ensure_config_creates_every_default_and_preserves_the_live_config() {
    let _lock = test_env_lock();
    let root = crate::test_support::fs::unique_temp_path("ensure-config");
    let xdg_root = root.join("xdg");
    let _xdg = EnvGuard::set("XDG_CONFIG_HOME", xdg_root.as_os_str());
    let _home = EnvGuard::set("HOME", root.join("home").as_os_str());
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let paths = test_paths(&root);
    let mut context = test_context(&detection, &paths);

    ensure_config(&mut context).expect("default config should be provisioned");

    let config_dir = xdg_root.join("unixnotis");
    let config_path = config_dir.join("config.toml");
    let config_text = fs::read_to_string(&config_path).expect("read generated config");
    toml::from_str::<Config>(&config_text).expect("generated config should parse");
    assert!(config_dir.join("installer.toml").is_file());
    for name in [
        "base.css",
        "panel.css",
        "popup.css",
        "widgets.css",
        "media.css",
    ] {
        assert!(config_dir.join(name).is_file(), "missing theme file {name}");
    }
    for script in unixnotis_core::DEFAULT_SCRIPTS {
        assert!(config_dir.join(script.relative_path).is_file());
    }

    fs::write(&config_path, "custom = true\n").expect("customize live config");
    ensure_config(&mut context).expect("existing config should be preserved");
    assert_eq!(
        fs::read_to_string(&config_path).expect("read retained config"),
        "custom = true\n"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn reset_config_backs_up_custom_files_and_restores_every_default() {
    let _lock = test_env_lock();
    let root = crate::test_support::fs::unique_temp_path("reset-config");
    let xdg_root = root.join("xdg");
    let _xdg = EnvGuard::set("XDG_CONFIG_HOME", xdg_root.as_os_str());
    let _home = EnvGuard::set("HOME", root.join("home").as_os_str());
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let paths = test_paths(&root);
    let mut context = test_context(&detection, &paths);
    ensure_config(&mut context).expect("seed default config");
    let config_dir = xdg_root.join("unixnotis");
    let config_path = config_dir.join("config.toml");
    fs::write(&config_path, "custom = true\n").expect("customize config");
    fs::write(config_dir.join("base.css"), "/* custom */\n").expect("customize theme");
    let script_path = config_dir.join(unixnotis_core::DEFAULT_SCRIPTS[0].relative_path);
    fs::write(&script_path, "#!/bin/sh\nexit 9\n").expect("customize script");

    reset_config(&mut context).expect("config reset should succeed");

    let config_text = fs::read_to_string(&config_path).expect("read reset config");
    toml::from_str::<Config>(&config_text).expect("reset config should parse");
    assert_ne!(config_text, "custom = true\n");
    assert_eq!(
        fs::read_to_string(config_dir.join("base.css")).expect("read reset theme"),
        unixnotis_core::DEFAULT_BASE_CSS
    );
    assert_eq!(
        fs::read_to_string(&script_path).expect("read reset script"),
        unixnotis_core::DEFAULT_SCRIPTS[0].contents
    );

    let backup_dir = fs::read_dir(&config_dir)
        .expect("read config directory")
        .filter_map(Result::ok)
        .find(|entry| {
            entry.file_type().is_ok_and(|kind| kind.is_dir())
                && entry.file_name().to_string_lossy().starts_with("Backup-")
        })
        .expect("reset should create a backup")
        .path();
    assert_eq!(
        fs::read_to_string(backup_dir.join("config.toml")).expect("read config backup"),
        "custom = true\n"
    );
    assert_eq!(
        fs::read_to_string(backup_dir.join("base.css")).expect("read theme backup"),
        "/* custom */\n"
    );
    let _ = fs::remove_dir_all(root);
}
