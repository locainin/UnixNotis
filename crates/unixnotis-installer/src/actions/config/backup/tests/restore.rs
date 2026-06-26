use super::super::{is_restore_target_allowed, restore_config};
use crate::detect::Detection;
use crate::events::UiMessage;
use crate::model::ActionMode;
use crate::paths::InstallPaths;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};

#[test]
fn restore_config_uses_restored_theme_paths() {
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
    fs::write(backup_dir.join("config.toml"), config_toml).expect("write config");
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
        restore_backup: Some(backup_dir.clone()),
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
    fs::write(backup_dir.join("config.toml"), config_toml).expect("write config");
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
        restore_backup: Some(backup_dir.clone()),
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
