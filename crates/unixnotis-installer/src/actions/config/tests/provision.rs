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
use unixnotis_core::{Config, ThemeMode};

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
fn ensure_config_uses_embedded_theme_and_preserves_the_live_config() {
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
        "theme.toml",
    ] {
        assert!(
            !config_dir.join(name).exists(),
            "new installs should not create custom theme file {name}"
        );
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
fn reset_config_backs_up_custom_files_and_selects_embedded_stock() {
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
    let reset = toml::from_str::<Config>(&config_text).expect("reset config should parse");
    assert_ne!(config_text, "custom = true\n");
    assert_eq!(reset.theme.mode, ThemeMode::Stock);
    assert_eq!(
        fs::read_to_string(config_dir.join("base.css")).expect("read reset theme"),
        "/* custom */\n",
        "reset must not convert embedded stock into a custom theme snapshot"
    );
    assert!(
        !config_dir.join("theme.toml").exists(),
        "ordinary reset must not materialize a stock theme manifest"
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

#[test]
fn installer_and_core_reset_wrappers_produce_the_same_files() {
    let _lock = test_env_lock();
    let root = crate::test_support::fs::unique_temp_path("reset-parity");
    let xdg_root = root.join("xdg");
    let _xdg = EnvGuard::set("XDG_CONFIG_HOME", xdg_root.as_os_str());
    let _home = EnvGuard::set("HOME", root.join("home").as_os_str());
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let paths = test_paths(&root);
    let mut context = test_context(&detection, &paths);
    let installer_dir = xdg_root.join("unixnotis");
    let core_dir = root.join("core-config");

    let seed = |directory: &std::path::Path| {
        fs::create_dir_all(directory.join("scripts")).expect("create reset fixture");
        fs::write(directory.join("config.toml"), "custom = true\n").expect("seed config");
        fs::write(directory.join("installer.toml"), "[backups]\nkeep = 3\n")
            .expect("seed settings");
        fs::write(directory.join("panel.css"), "custom panel\n").expect("seed theme");
        fs::write(
            directory.join(unixnotis_core::DEFAULT_SCRIPTS[0].relative_path),
            "custom script\n",
        )
        .expect("seed script");
    };
    seed(&installer_dir);
    seed(&core_dir);

    reset_config(&mut context).expect("installer reset should succeed");
    unixnotis_core::reset_config_to_defaults(&unixnotis_core::ResetConfigOptions {
        config_dir: core_dir.clone(),
        backup_retention: 3,
    })
    .expect("core reset should succeed");

    for relative in [
        "config.toml",
        "panel.css",
        "scripts/unixnotis-blue-light-state",
    ] {
        assert_eq!(
            fs::read(installer_dir.join(relative)).expect("read installer result"),
            fs::read(core_dir.join(relative)).expect("read core result"),
            "reset wrappers must write the same {relative}"
        );
    }
    let installer_backup = fs::read_dir(&installer_dir)
        .expect("read installer backups")
        .filter_map(Result::ok)
        .find(|entry| entry.file_name().to_string_lossy().starts_with("Backup-"))
        .expect("installer backup");
    let core_backup = fs::read_dir(&core_dir)
        .expect("read core backups")
        .filter_map(Result::ok)
        .find(|entry| entry.file_name().to_string_lossy().starts_with("Backup-"))
        .expect("core backup");
    for name in ["config.toml", "panel.css"] {
        assert_eq!(
            fs::read(installer_backup.path().join(name)).expect("read installer backup"),
            fs::read(core_backup.path().join(name)).expect("read core backup"),
        );
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn reset_rejects_invalid_installer_settings_without_changes() {
    let _lock = test_env_lock();
    let root = crate::test_support::fs::unique_temp_path("reset-invalid-settings");
    let xdg_root = root.join("xdg");
    let _xdg = EnvGuard::set("XDG_CONFIG_HOME", xdg_root.as_os_str());
    let _home = EnvGuard::set("HOME", root.join("home").as_os_str());
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let paths = test_paths(&root);
    let mut context = test_context(&detection, &paths);
    ensure_config(&mut context).expect("seed reset fixture");

    let config_dir = xdg_root.join("unixnotis");
    let config_path = config_dir.join("config.toml");
    let script_path = config_dir.join(unixnotis_core::DEFAULT_SCRIPTS[0].relative_path);
    fs::write(&config_path, "custom config\n").expect("customize config");
    fs::write(config_dir.join("panel.css"), "custom panel\n").expect("customize theme");
    fs::write(&script_path, "custom script\n").expect("customize script");
    fs::write(config_dir.join("installer.toml"), "[backups\n").expect("corrupt installer settings");
    let before_config = fs::read(&config_path).expect("read config before reset");
    let before_theme = fs::read(config_dir.join("panel.css")).expect("read theme before reset");
    let before_script = fs::read(&script_path).expect("read script before reset");

    let error = reset_config(&mut context).expect_err("invalid settings must abort reset");

    assert!(error.to_string().contains("installer settings"));
    assert_eq!(
        fs::read(&config_path).expect("read config after reset"),
        before_config
    );
    assert_eq!(
        fs::read(config_dir.join("panel.css")).expect("read theme after reset"),
        before_theme
    );
    assert_eq!(
        fs::read(&script_path).expect("read script after reset"),
        before_script
    );
    assert_eq!(
        fs::read_dir(&config_dir)
            .expect("read reset directory")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("Backup-"))
            .count(),
        0,
        "invalid settings must not create a backup"
    );
    let _ = fs::remove_dir_all(root);
}
