use super::super::restore::{is_restore_target_allowed, restore_config};
use crate::app::events::UiMessage;
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
    let paths = InstallPaths::discover().expect("paths should resolve in repo tests");
    let (tx, _rx) = mpsc::sync_channel::<UiMessage>(8);
    let mut ctx = crate::actions::ActionContext {
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
    let config_dir = std::env::temp_dir().join("unixnotis-restore-guard");
    let inside = config_dir.join("themes/base.css");
    let outside = std::env::temp_dir().join("unixnotis-escape.css");
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

    let paths = InstallPaths::discover().expect("paths should resolve in repo tests");
    let (tx, _rx) = mpsc::sync_channel::<UiMessage>(8);
    let mut ctx = crate::actions::ActionContext {
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
    fs::write(config_dir.join("config.toml"), current_config_text("")).expect("write config");

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

    let paths = InstallPaths::discover().expect("paths should resolve in repo tests");
    let (tx, _rx) = mpsc::sync_channel::<UiMessage>(16);
    let mut ctx = crate::actions::ActionContext {
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

#[test]
fn malformed_backup_config_fails_before_any_live_file_changes() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = std::env::temp_dir().join(format!(
        "unixnotis-invalid-restore-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos()
    ));
    let config_dir = root.join("unixnotis");
    let backup_dir = config_dir.join("Backup-invalid");
    fs::create_dir_all(&backup_dir).expect("create backup directory");
    fs::write(
        config_dir.join("config.toml"),
        current_config_text("[theme]\nbase_css = \"live.css\"\n"),
    )
    .expect("write live config");
    fs::write(config_dir.join("live.css"), "live theme\n").expect("write live theme");
    fs::write(backup_dir.join("config.toml"), "config_version = 5\n[")
        .expect("write malformed backup config");
    fs::write(backup_dir.join("base.css"), "backup theme\n").expect("write backup theme");
    let before = snapshot_tree(&config_dir);

    let paths = InstallPaths::discover().expect("paths should resolve in repo tests");
    let (tx, _rx) = mpsc::sync_channel::<UiMessage>(8);
    let mut ctx = crate::actions::ActionContext {
        paths: &paths,
        install_state: None,
        log_tx: tx,
        action_mode: ActionMode::Install,
        restore_backup: Some(backup_dir),
        service_reload_required: Arc::new(AtomicBool::new(false)),
    };

    let error = restore_config(&mut ctx).expect_err("malformed backup must fail closed");

    assert!(error.to_string().contains("not valid schema v5"));
    assert_eq!(
        snapshot_tree(&config_dir),
        before,
        "validation failure must leave the complete live tree unchanged"
    );
    fs::remove_dir_all(root).expect("remove restore test root");
}

#[test]
fn restore_target_snapshot_failure_happens_before_any_file_is_published() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = crate::test_support::fs::unique_temp_path("restore-snapshot-rollback");
    let config_dir = root.join("unixnotis");
    let backup_dir = config_dir.join("Backup-snapshot-rollback");
    fs::create_dir_all(&backup_dir).expect("create backup directory");
    let original_config = current_config_text("[theme]\npanel_css = \"live-panel.css\"\n");
    fs::write(config_dir.join("config.toml"), &original_config).expect("write live config");
    fs::write(config_dir.join("live-panel.css"), "live panel\n").expect("write live panel");
    fs::create_dir(config_dir.join("blocked-panel.css")).expect("create invalid target directory");
    fs::write(
        backup_dir.join("config.toml"),
        current_config_text("[theme]\npanel_css = \"blocked-panel.css\"\n"),
    )
    .expect("write backup config");
    fs::write(backup_dir.join("panel.css"), "restored panel\n").expect("write backup panel");
    let before = snapshot_tree(&config_dir);

    let paths = InstallPaths::discover().expect("paths should resolve in repo tests");
    let (tx, _rx) = mpsc::sync_channel::<UiMessage>(8);
    let mut ctx = crate::actions::ActionContext {
        paths: &paths,
        install_state: None,
        log_tx: tx,
        action_mode: ActionMode::Install,
        restore_backup: Some(backup_dir),
        service_reload_required: Arc::new(AtomicBool::new(false)),
    };

    let error = restore_config(&mut ctx)
        .expect_err("an invalid later target must roll back an earlier config replacement");

    assert!(error
        .to_string()
        .contains("restore target is not a regular file"));
    assert_eq!(
        snapshot_tree(&config_dir),
        before,
        "snapshot failure must happen before any live file is published"
    );
    fs::remove_dir_all(root).expect("remove restore rollback fixture");
}

fn snapshot_tree(root: &std::path::Path) -> Vec<(PathBuf, Vec<u8>, u32)> {
    fn visit(
        root: &std::path::Path,
        directory: &std::path::Path,
        snapshot: &mut Vec<(PathBuf, Vec<u8>, u32)>,
    ) {
        let mut entries = fs::read_dir(directory)
            .expect("read snapshot directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect snapshot entries");
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).expect("snapshot metadata");
            if metadata.is_dir() {
                visit(root, &path, snapshot);
            } else {
                snapshot.push((
                    path.strip_prefix(root)
                        .expect("snapshot relative path")
                        .to_path_buf(),
                    fs::read(&path).expect("snapshot file"),
                    metadata.permissions().mode() & 0o777,
                ));
            }
        }
    }

    let mut snapshot = Vec::new();
    visit(root, root, &mut snapshot);
    snapshot
}
