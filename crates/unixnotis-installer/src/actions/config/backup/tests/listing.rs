use std::fs;
use std::path::PathBuf;

use super::super::listing::list_backup_dirs;

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

    assert_eq!(backups, vec![root.join("Backup-2026-06-01")]);
    let _ = fs::remove_dir_all(&root);
}
