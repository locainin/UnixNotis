use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};

use crate::detect::Detection;

use super::super::snapshot::backup_existing_file;
use super::support::{test_context, test_paths};

#[test]
fn backup_snapshot_copies_contents_and_source_mode() {
    let root = crate::test_support::fs::unique_temp_path("backup-snapshot-copy");
    let source = root.join("config.toml");
    let backup_dir = root.join("Backup-2026-07-22");
    fs::create_dir_all(&backup_dir).expect("create backup directory");
    fs::write(&source, "private config\n").expect("write source config");
    fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).expect("set source mode");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let mut context = test_context(&detection, &paths);

    backup_existing_file(&mut context, &source, "config.toml", Some(&backup_dir))
        .expect("create backup snapshot");

    let snapshot = backup_dir.join("config.toml");
    assert_eq!(
        fs::read_to_string(&snapshot).expect("read backup snapshot"),
        "private config\n"
    );
    assert_eq!(
        fs::metadata(snapshot)
            .expect("snapshot metadata")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn backup_snapshot_rejects_destination_symlink_without_changing_target() {
    let root = crate::test_support::fs::unique_temp_path("backup-snapshot-symlink");
    let source = root.join("config.toml");
    let backup_dir = root.join("Backup-2026-07-22");
    let protected = root.join("protected");
    fs::create_dir_all(&backup_dir).expect("create backup directory");
    fs::write(&source, "new config\n").expect("write source config");
    fs::write(&protected, "protected\n").expect("write protected file");
    symlink(&protected, backup_dir.join("config.toml")).expect("create snapshot link");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let mut context = test_context(&detection, &paths);

    backup_existing_file(&mut context, &source, "config.toml", Some(&backup_dir))
        .expect_err("snapshot destination link should fail");

    assert_eq!(
        fs::read_to_string(&protected).expect("read protected file"),
        "protected\n"
    );
    assert!(fs::symlink_metadata(backup_dir.join("config.toml"))
        .expect("snapshot link remains")
        .file_type()
        .is_symlink());
    let _ = fs::remove_dir_all(root);
}
