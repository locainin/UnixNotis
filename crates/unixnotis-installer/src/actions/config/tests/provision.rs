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
use crate::test_support::current_config_text;
use crate::test_support::env::{test_env_lock, EnvGuard};
use unixnotis_core::{
    Config, DEFAULT_BASE_CSS, DEFAULT_MEDIA_CSS, DEFAULT_PANEL_CSS, DEFAULT_POPUP_CSS,
    DEFAULT_WIDGETS_CSS,
};

use super::super::provision::{
    classify_external_theme_file, ensure_config, reset_config, ThemeFileStatus,
};

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
fn ensure_config_provisions_default_css_and_preserves_the_live_config() {
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
    for (name, expected) in [
        ("base.css", DEFAULT_BASE_CSS),
        ("panel.css", DEFAULT_PANEL_CSS),
        ("popup.css", DEFAULT_POPUP_CSS),
        ("widgets.css", DEFAULT_WIDGETS_CSS),
        ("media.css", DEFAULT_MEDIA_CSS),
    ] {
        assert!(
            config_dir.join(name).is_file(),
            "new installs should create {name}"
        );
        assert_eq!(
            fs::read_to_string(config_dir.join(name)).expect("read default theme CSS"),
            expected,
            "new installs should use bundled {name}"
        );
    }
    assert!(!config_dir.join("theme.toml").exists());
    for script in unixnotis_core::DEFAULT_SCRIPTS {
        assert!(config_dir.join(script.relative_path).is_file());
    }

    fs::write(&config_path, current_config_text("custom = true\n")).expect("customize live config");
    fs::write(config_dir.join("popup.css"), "/* custom popup */\n").expect("customize popup CSS");
    ensure_config(&mut context).expect("existing config should be preserved");
    assert_eq!(
        fs::read_to_string(&config_path).expect("read retained config"),
        current_config_text("custom = true\n")
    );
    assert_eq!(
        fs::read_to_string(config_dir.join("popup.css")).expect("read retained popup CSS"),
        "/* custom popup */\n"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn ensure_config_provisions_the_existing_configured_theme_paths() {
    let _lock = test_env_lock();
    let root = crate::test_support::fs::unique_temp_path("ensure-configured-theme-paths");
    let xdg_root = root.join("xdg");
    let _xdg = EnvGuard::set("XDG_CONFIG_HOME", xdg_root.as_os_str());
    let _home = EnvGuard::set("HOME", root.join("home").as_os_str());
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let paths = test_paths(&root);
    let mut context = test_context(&detection, &paths);
    let config_dir = xdg_root.join("unixnotis");
    fs::create_dir_all(&config_dir).expect("create config directory");
    fs::write(
        config_dir.join("config.toml"),
        current_config_text(
            "[theme]\nbase_css = \"themes/base.css\"\npanel_css = \"themes/panel.css\"\npopup_css = \"themes/popup.css\"\nwidgets_css = \"themes/widgets.css\"\nmedia_css = \"themes/media.css\"\n",
        ),
    )
    .expect("write configured theme paths");
    fs::create_dir_all(config_dir.join("themes")).expect("create configured theme directory");
    fs::write(config_dir.join("themes/popup.css"), "/* custom popup */\n")
        .expect("seed custom configured popup");

    ensure_config(&mut context).expect("configured theme paths should be provisioned");

    for (name, expected) in [
        ("base.css", DEFAULT_BASE_CSS),
        ("panel.css", DEFAULT_PANEL_CSS),
        ("popup.css", "/* custom popup */\n"),
        ("widgets.css", DEFAULT_WIDGETS_CSS),
        ("media.css", DEFAULT_MEDIA_CSS),
    ] {
        assert_eq!(
            fs::read_to_string(config_dir.join("themes").join(name))
                .expect("read configured theme file"),
            expected,
            "configured theme paths must be the provisioned targets"
        );
        assert!(
            !config_dir.join(name).exists(),
            "installer must not create unrelated root-level {name}"
        );
    }
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn ensure_config_preserves_external_theme_files_without_creating_missing_or_unsafe_targets() {
    use std::os::unix::fs::symlink;

    let _lock = test_env_lock();
    let root = crate::test_support::fs::unique_temp_path("ensure-external-theme-paths");
    let xdg_root = root.join("xdg");
    let _xdg = EnvGuard::set("XDG_CONFIG_HOME", xdg_root.as_os_str());
    let _home = EnvGuard::set("HOME", root.join("home").as_os_str());
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let paths = test_paths(&root);
    let mut context = test_context(&detection, &paths);
    let config_dir = xdg_root.join("unixnotis");
    fs::create_dir_all(&config_dir).expect("create config directory");
    let external_root = root.join("external-theme");
    fs::create_dir_all(&external_root).expect("create external theme directory");
    let external_base = external_root.join("base.css");
    let external_popup = external_root.join("popup.css");
    let external_panel = external_root.join("panel.css");
    let external_widgets = external_root.join("widgets.css");
    let external_media = external_root.join("media.css");
    fs::write(&external_base, "/* external base */\n").expect("seed external base");
    fs::write(&external_popup, "/* external popup */\n").expect("seed external popup");
    let external_target = root.join("external-target.css");
    fs::write(&external_target, "/* external target */\n").expect("seed external target");
    symlink(&external_target, &external_widgets).expect("create external symlink");
    fs::create_dir(&external_media).expect("create external special target");
    fs::write(
        config_dir.join("config.toml"),
        current_config_text(&format!(
            "[theme]\nbase_css = {:?}\npopup_css = {:?}\npanel_css = {:?}\nwidgets_css = {:?}\nmedia_css = {:?}\n",
            external_base.to_string_lossy(),
            external_popup.to_string_lossy(),
            external_panel.to_string_lossy(),
            external_widgets.to_string_lossy(),
            external_media.to_string_lossy(),
        )),
    )
    .expect("write external theme paths");

    ensure_config(&mut context).expect("external theme paths must remain compatible");

    assert_eq!(
        fs::read_to_string(&external_base).expect("read external base"),
        "/* external base */\n"
    );
    assert_eq!(
        fs::read_to_string(&external_popup).expect("read external popup"),
        "/* external popup */\n"
    );
    assert!(
        !external_panel.exists(),
        "missing external files must stay absent"
    );
    assert!(
        external_media.is_dir(),
        "external directories must remain intact"
    );
    assert_eq!(
        fs::read_link(&external_widgets).expect("read external symlink"),
        external_target
    );
    assert_eq!(
        fs::read_to_string(&external_target).expect("read external symlink target"),
        "/* external target */\n"
    );
    for name in [
        "base.css",
        "panel.css",
        "popup.css",
        "widgets.css",
        "media.css",
    ] {
        assert!(
            !config_dir.join(name).exists(),
            "external theme paths must not create root-level {name}"
        );
    }
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn external_theme_file_status_matches_runtime_file_safety() {
    use std::os::unix::fs::symlink;

    let root = crate::test_support::fs::unique_temp_path("external-theme-status");
    fs::create_dir_all(&root).expect("create status fixture");
    let missing = root.join("missing.css");
    let regular = root.join("regular.css");
    let directory = root.join("directory.css");
    let symlink_path = root.join("symlink.css");
    let target = root.join("target.css");
    fs::write(&regular, "/* regular */\n").expect("seed regular file");
    fs::create_dir(&directory).expect("seed directory target");
    fs::write(&target, "/* target */\n").expect("seed symlink target");
    symlink(&target, &symlink_path).expect("seed symlink target");

    assert_eq!(
        classify_external_theme_file(&missing),
        ThemeFileStatus::ExternalMissing
    );
    assert_eq!(
        classify_external_theme_file(&regular),
        ThemeFileStatus::ExternalManaged
    );
    assert_eq!(
        classify_external_theme_file(&directory),
        ThemeFileStatus::ExternalUnsafe
    );
    assert_eq!(
        classify_external_theme_file(&symlink_path),
        ThemeFileStatus::ExternalUnsafe
    );

    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn ensure_config_rejects_theme_symlinks_without_touching_the_target() {
    use std::os::unix::fs::symlink;

    let _lock = test_env_lock();
    let root = crate::test_support::fs::unique_temp_path("ensure-theme-symlink");
    let xdg_root = root.join("xdg");
    let _xdg = EnvGuard::set("XDG_CONFIG_HOME", xdg_root.as_os_str());
    let _home = EnvGuard::set("HOME", root.join("home").as_os_str());
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let paths = test_paths(&root);
    let mut context = test_context(&detection, &paths);
    ensure_config(&mut context).expect("initial install should succeed");

    let config_dir = xdg_root.join("unixnotis");
    let target = root.join("outside-popup.css");
    fs::write(&target, "/* outside target */\n").expect("seed outside CSS");
    fs::remove_file(config_dir.join("popup.css")).expect("remove provisioned popup CSS");
    symlink(&target, config_dir.join("popup.css")).expect("create popup symlink");

    ensure_config(&mut context).expect_err("theme symlink should fail closed");

    assert_eq!(
        fs::read_to_string(&target).expect("read outside CSS target"),
        "/* outside target */\n"
    );
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn ensure_config_rejects_configured_theme_symlinks_without_touching_the_target() {
    use std::os::unix::fs::symlink;

    let _lock = test_env_lock();
    let root = crate::test_support::fs::unique_temp_path("ensure-configured-theme-symlink");
    let xdg_root = root.join("xdg");
    let _xdg = EnvGuard::set("XDG_CONFIG_HOME", xdg_root.as_os_str());
    let _home = EnvGuard::set("HOME", root.join("home").as_os_str());
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let paths = test_paths(&root);
    let mut context = test_context(&detection, &paths);
    let config_dir = xdg_root.join("unixnotis");
    fs::create_dir_all(config_dir.join("themes")).expect("create configured theme directory");
    fs::write(
        config_dir.join("config.toml"),
        current_config_text("[theme]\npopup_css = \"themes/popup.css\"\n"),
    )
    .expect("write configured popup path");
    let target = root.join("outside-popup.css");
    fs::write(&target, "/* outside target */\n").expect("seed outside CSS");
    symlink(&target, config_dir.join("themes/popup.css")).expect("create configured symlink");

    ensure_config(&mut context).expect_err("configured theme symlink should fail closed");

    assert_eq!(
        fs::read_to_string(&target).expect("read outside CSS target"),
        "/* outside target */\n"
    );
    assert_eq!(
        fs::read_link(config_dir.join("themes/popup.css")).expect("read retained symlink"),
        target
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn reset_config_backs_up_custom_files_and_restores_configured_css_defaults() {
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
    assert_eq!(reset.theme.base_css, "base.css");
    assert_eq!(
        fs::read_to_string(config_dir.join("base.css")).expect("read reset theme"),
        DEFAULT_BASE_CSS,
        "reset must restore the active configured stylesheet"
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

#[test]
fn reset_rejects_non_file_installer_settings_without_changes() {
    let _lock = test_env_lock();
    let root = crate::test_support::fs::unique_temp_path("reset-directory-settings");
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
    fs::write(&config_path, "custom config\n").expect("customize config");
    fs::remove_file(config_dir.join("installer.toml")).expect("remove settings file");
    fs::create_dir(config_dir.join("installer.toml")).expect("create settings directory");
    let before_config = fs::read(&config_path).expect("read config before reset");

    let error = reset_config(&mut context).expect_err("directory settings must abort reset");

    assert!(error.to_string().contains("installer settings"));
    assert_eq!(
        fs::read(&config_path).expect("read config after reset"),
        before_config
    );
    assert_eq!(
        fs::read_dir(&config_dir)
            .expect("read reset directory")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("Backup-"))
            .count(),
        0,
        "non-file settings must not create a backup"
    );
    let _ = fs::remove_dir_all(root);
}
