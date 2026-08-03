use std::fs;

use super::super::{reset_config_to_defaults, ResetConfigOptions};
use super::support::temp_config_dir;

#[test]
fn reset_retains_only_the_newest_backup_directory() {
    let root = temp_config_dir("retention");
    for name in ["Backup-2026-07-30-120000", "Backup-2026-07-31-120000"] {
        fs::create_dir(root.join(name)).expect("seed old backup");
    }

    let report = reset_config_to_defaults(&ResetConfigOptions {
        config_dir: root.clone(),
        backup_retention: 1,
    })
    .expect("reset should succeed");

    let backups = fs::read_dir(&root)
        .expect("read config directory")
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("Backup-"))
        .count();
    assert_eq!(backups, 1);
    assert!(report.backup_dir.is_some());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn retention_ignores_backup_named_regular_files() {
    let root = temp_config_dir("retention-file");
    for name in ["Backup-2026-07-30-120000", "Backup-2026-07-31-120000"] {
        fs::create_dir(root.join(name)).expect("seed old backup");
    }
    let regular_backup = root.join("Backup-z-not-a-directory");
    fs::write(&regular_backup, b"keep this file").expect("seed regular backup-like file");

    reset_config_to_defaults(&ResetConfigOptions {
        config_dir: root.clone(),
        backup_retention: 2,
    })
    .expect("reset should succeed");

    assert!(regular_backup.is_file());
    assert!(root.join("Backup-2026-07-31-120000").is_dir());
    let _ = fs::remove_dir_all(root);
}
