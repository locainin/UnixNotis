use super::super::restore::{is_restore_target_allowed, restore_config};
use crate::app::events::UiMessage;
use crate::detect::Detection;
use crate::model::ActionMode;
use crate::paths::InstallPaths;
use crate::test_support::current_config_text;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};
use unixnotis_core::{reset_config_to_defaults, ResetConfigOptions, DEFAULT_SCRIPTS};

#[test]
fn restore_config_uses_restored_theme_paths() {
    let _lock = crate::test_support::env::test_env_lock();
    // Simulate a backup that points theme files into a custom relative folder
    let root = PathBuf::from("target").join(format!(
        "unixnotis-installer-restore-test-{}",
        std::process::id()
    ));
    let config_dir = root.join("unixnotis");
    let _ = fs::create_dir_all(&config_dir);
    let backup_dir = config_dir.join("Backup-2024-01-01");
    let _ = fs::create_dir_all(&backup_dir);

    let config_toml = r#"
[theme]
base_css = "themes/custom/base.css"
panel_css = "themes/custom/panel.css"
popup_css = "themes/custom/popup.css"
widgets_css = "themes/custom/widgets.css"
media_css = "themes/custom/media.css"
"#;
    fs::write(
        backup_dir.join("config.toml"),
        current_config_text(config_toml),
    )
    .expect("write config");
    fs::write(backup_dir.join("base.css"), "base").expect("write base");
    fs::write(backup_dir.join("panel.css"), "panel").expect("write panel");
    fs::write(backup_dir.join("popup.css"), "popup").expect("write popup");
    fs::write(backup_dir.join("widgets.css"), "widgets").expect("write widgets");
    fs::write(backup_dir.join("media.css"), "media").expect("write media");

    // Restore path selection is driven through ActionContext just like runtime
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let paths = InstallPaths::discover().expect("paths should resolve in repo tests");
    let (tx, _rx) = mpsc::sync_channel::<UiMessage>(8);
    let mut ctx = crate::actions::ActionContext {
        detection: &detection,
        paths: &paths,
        install_state: None,
        log_tx: tx,
        action_mode: ActionMode::Install,
        restore_backup: Some(backup_dir),
        service_reload_required: Arc::new(AtomicBool::new(false)),
    };

    restore_config(&mut ctx).expect("restore should succeed");

    // Restored config drives target resolution for every theme file
    let config_path = config_dir.join("config.toml");
    assert!(config_path.exists());
    let custom_base = config_dir.join("themes").join("custom").join("base.css");
    let custom_panel = config_dir.join("themes").join("custom").join("panel.css");
    let custom_popup = config_dir.join("themes").join("custom").join("popup.css");
    let custom_widgets = config_dir.join("themes").join("custom").join("widgets.css");
    let custom_media = config_dir.join("themes").join("custom").join("media.css");
    assert!(custom_base.exists());
    assert!(custom_panel.exists());
    assert!(custom_popup.exists());
    assert!(custom_widgets.exists());
    assert!(custom_media.exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn restore_target_guard_blocks_paths_outside_config_dir() {
    // Guard should allow in-tree writes and reject out-of-tree targets
    let config_dir = PathBuf::from("/tmp/unixnotis-restore-guard");
    let inside = config_dir.join("themes/base.css");
    let outside = PathBuf::from("/tmp/unixnotis-escape.css");
    assert!(is_restore_target_allowed(&config_dir, &inside));
    assert!(!is_restore_target_allowed(&config_dir, &outside));
}

#[test]
fn restore_config_skips_absolute_theme_targets() {
    let _lock = crate::test_support::env::test_env_lock();
    // Backup contains an absolute base_css path that must be ignored
    let root = PathBuf::from("target").join(format!(
        "unixnotis-installer-restore-guard-test-{}",
        std::process::id()
    ));
    let config_dir = root.join("unixnotis");
    let _ = fs::create_dir_all(&config_dir);
    let backup_dir = config_dir.join("Backup-2024-01-02");
    let _ = fs::create_dir_all(&backup_dir);
    let escaped_target = std::env::temp_dir().join(format!(
        "unixnotis-restore-escape-{}.css",
        std::process::id()
    ));
    let _ = fs::remove_file(&escaped_target);

    let config_toml = format!(
        "[theme]\nbase_css = \"{}\"\npanel_css = \"panel.css\"\npopup_css = \"popup.css\"\nwidgets_css = \"widgets.css\"\nmedia_css = \"media.css\"\n",
        escaped_target.display()
    );
    fs::write(
        backup_dir.join("config.toml"),
        current_config_text(&config_toml),
    )
    .expect("write config");
    fs::write(backup_dir.join("base.css"), "base").expect("write base");
    fs::write(backup_dir.join("panel.css"), "panel").expect("write panel");
    fs::write(backup_dir.join("popup.css"), "popup").expect("write popup");
    fs::write(backup_dir.join("widgets.css"), "widgets").expect("write widgets");
    fs::write(backup_dir.join("media.css"), "media").expect("write media");

    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let paths = InstallPaths::discover().expect("paths should resolve in repo tests");
    let (tx, _rx) = mpsc::sync_channel::<UiMessage>(8);
    let mut ctx = crate::actions::ActionContext {
        detection: &detection,
        paths: &paths,
        install_state: None,
        log_tx: tx,
        action_mode: ActionMode::Install,
        restore_backup: Some(backup_dir),
        service_reload_required: Arc::new(AtomicBool::new(false)),
    };

    restore_config(&mut ctx).expect("restore should succeed");

    // Absolute escape target stays untouched while safe theme files restore
    assert!(
        !escaped_target.exists(),
        "restore must not write outside config dir"
    );
    assert!(config_dir.join("panel.css").exists());
    assert!(config_dir.join("popup.css").exists());
    assert!(config_dir.join("widgets.css").exists());
    assert!(config_dir.join("media.css").exists());

    let _ = fs::remove_file(&escaped_target);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn restore_config_restores_all_bundled_scripts_and_executable_modes() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = PathBuf::from("target").join(format!(
        "unixnotis-installer-script-restore-test-{}",
        std::process::id()
    ));
    let config_dir = root.join("unixnotis");
    fs::create_dir_all(config_dir.join("scripts")).expect("create script directory");
    fs::write(config_dir.join("config.toml"), "custom = true\n").expect("write config");

    // Seed every bundled script with distinct user content and non-default permissions
    for (index, script) in DEFAULT_SCRIPTS.iter().enumerate() {
        let path = config_dir.join(script.relative_path);
        fs::write(&path, format!("custom script {index}\n")).expect("write custom script");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
            .expect("set custom script mode");
    }

    let report = reset_config_to_defaults(&ResetConfigOptions {
        config_dir: config_dir.clone(),
        backup_retention: 1,
    })
    .expect("reset should create a restorable script backup");
    let backup_dir = report.backup_dir.expect("reset backup directory");

    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let paths = InstallPaths::discover().expect("paths should resolve in repo tests");
    let (tx, _rx) = mpsc::sync_channel::<UiMessage>(16);
    let mut ctx = crate::actions::ActionContext {
        detection: &detection,
        paths: &paths,
        install_state: None,
        log_tx: tx,
        action_mode: ActionMode::Install,
        restore_backup: Some(backup_dir),
        service_reload_required: Arc::new(AtomicBool::new(false)),
    };

    restore_config(&mut ctx).expect("restore should restore bundled scripts");

    for (index, script) in DEFAULT_SCRIPTS.iter().enumerate() {
        let path = config_dir.join(script.relative_path);
        assert_eq!(
            fs::read_to_string(&path).expect("read restored script"),
            format!("custom script {index}\n")
        );
        assert_eq!(
            fs::metadata(&path)
                .expect("restored script metadata")
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
    }

    let _ = fs::remove_dir_all(&root);
}
