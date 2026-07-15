use super::super::{create_backup_dir, list_backup_dirs, prune_old_backups, BackupConfig};
use crate::app::events::UiMessage;
use crate::detect::Detection;
use crate::model::ActionMode;
use crate::paths::InstallPaths;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};

#[test]
fn prune_old_backups_keeps_newest() {
    let _lock = crate::test_support::env::test_env_lock();
    // Backup names are date-ordered, so lexical sort can drive retention
    let root = PathBuf::from("target").join(format!(
        "unixnotis-installer-backup-prune-test-{}",
        std::process::id()
    ));
    let _ = fs::create_dir_all(&root);
    let names = [
        "Backup-2024-01-01",
        "Backup-2024-01-02",
        "Backup-2024-01-03",
        "Backup-2024-01-04",
    ];
    for name in names {
        let _ = fs::create_dir_all(root.join(name));
    }

    // Minimal installer context for pruning logic
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
        restore_backup: None,
        service_reload_required: Arc::new(AtomicBool::new(false)),
    };
    prune_old_backups(&mut ctx, &root, 2).expect("prune should succeed");

    // Only the two newest entries should remain
    let mut remaining = list_backup_dirs(&root)
        .into_iter()
        .map(|path: std::path::PathBuf| {
            path.file_name()
                .expect("backup directory should have a file name")
                .to_string_lossy()
                .to_string()
        })
        .collect::<Vec<_>>();
    remaining.sort();
    assert_eq!(
        remaining,
        vec![
            "Backup-2024-01-03".to_string(),
            "Backup-2024-01-04".to_string()
        ]
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn backup_config_defaults_to_three() {
    // Default retention should match installer template behavior
    let config = BackupConfig::default();
    assert_eq!(config.keep, 3);
}

#[test]
fn create_backup_dir_keeps_new_directory_when_retention_is_full() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = PathBuf::from("target").join(format!(
        "unixnotis-installer-backup-create-test-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let _ = fs::create_dir_all(&root);
    for name in [
        "Backup-2026-05-31-003",
        "Backup-2026-05-31-004",
        "Backup-2026-05-31-005",
    ] {
        let _ = fs::create_dir_all(root.join(name));
    }

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
        restore_backup: None,
        service_reload_required: Arc::new(AtomicBool::new(false)),
    };

    let backup_dir = create_backup_dir(&mut ctx, &root, 3)
        .expect("backup directory should be created")
        .expect("backups should be enabled");

    assert!(
        backup_dir.exists(),
        "new backup directory must survive retention pruning"
    );
    assert_eq!(list_backup_dirs(&root).len(), 3);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn create_backup_dir_returns_none_when_retention_is_disabled() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = PathBuf::from("target").join(format!(
        "unixnotis-installer-backup-disabled-test-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let _ = fs::create_dir_all(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let paths = InstallPaths::discover().expect("paths should resolve in repo tests");
    let (tx, rx) = mpsc::sync_channel::<UiMessage>(8);
    let mut ctx = crate::actions::ActionContext {
        detection: &detection,
        paths: &paths,
        install_state: None,
        log_tx: tx,
        action_mode: ActionMode::Install,
        restore_backup: None,
        service_reload_required: Arc::new(AtomicBool::new(false)),
    };

    let backup = create_backup_dir(&mut ctx, &root, 0).expect("disabled backups should succeed");

    // keep = 0 is an explicit opt-out and must not create a backup directory
    assert!(backup.is_none());
    assert!(list_backup_dirs(&root).is_empty());
    let log = rx.try_recv().expect("disabled backup log");
    assert!(matches!(
        log,
        UiMessage::Worker(crate::app::events::WorkerEvent::LogLine(message))
            if message.contains("Backups disabled")
    ));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn list_backup_dirs_filters_non_backup_entries_and_files() {
    let root = PathBuf::from("target").join(format!(
        "unixnotis-installer-backup-list-test-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let _ = fs::create_dir_all(&root);
    fs::create_dir_all(root.join("Backup-2026-06-01")).expect("backup dir");
    fs::create_dir_all(root.join("Other-2026-06-01")).expect("foreign dir");
    fs::write(root.join("Backup-2026-06-02"), "not a dir").expect("backup-like file");

    let backups = list_backup_dirs(&root);

    // Restore UI must show only installer backup directories, not similarly named files
    assert_eq!(backups, vec![root.join("Backup-2026-06-01")]);
    let _ = fs::remove_dir_all(&root);
}
