//! Regression tests for exact-byte stock theme migration

use std::fs;
use std::io;

use super::super::theme_stock::{
    migrate_known_stock_file, migrate_stock_file_with_writer, stock_backup_path,
};
use super::support::test_root;

const OLD_STOCK: &[u8] = b"/* exact previous stock */\n.card { color: red; }\n";
const CURRENT_STOCK: &[u8] = b"/* current flattened stock */\n.card { color: blue; }\n";
const BACKUP_TAG: &str = "unixnotis-stock-test";

#[test]
fn exact_legacy_stock_file_is_backed_up_and_atomically_migrated() {
    let root = test_root("exact-stock-migration");
    fs::create_dir_all(&root).expect("theme root");
    let target = root.join("panel.css");
    fs::write(&target, OLD_STOCK).expect("legacy stock");

    let migrated = migrate(&target, OLD_STOCK).expect("stock migration");

    assert!(migrated);
    assert_eq!(fs::read(&target).expect("current stock"), CURRENT_STOCK);
    let backup = stock_backup_path(&target, BACKUP_TAG).expect("backup path");
    assert_eq!(fs::read(backup).expect("stock backup"), OLD_STOCK);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn one_byte_stock_modification_prevents_automatic_replacement() {
    let root = test_root("modified-stock-preserved");
    fs::create_dir_all(&root).expect("theme root");
    let target = root.join("widgets.css");
    let mut customized = OLD_STOCK.to_vec();
    customized.push(b' ');
    fs::write(&target, &customized).expect("customized stock");

    let migrated = migrate(&target, OLD_STOCK).expect("migration check");

    assert!(!migrated);
    assert_eq!(fs::read(&target).expect("customized stock"), customized);
    let backup = stock_backup_path(&target, BACKUP_TAG).expect("backup path");
    assert!(!backup.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn current_stock_file_remains_unchanged() {
    let root = test_root("current-stock-preserved");
    fs::create_dir_all(&root).expect("theme root");
    let target = root.join("media.css");
    fs::write(&target, CURRENT_STOCK).expect("current stock");

    let migrated = migrate(&target, OLD_STOCK).expect("migration check");

    assert!(!migrated);
    assert_eq!(fs::read(&target).expect("current stock"), CURRENT_STOCK);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn interrupted_replacement_keeps_complete_legacy_file_and_backup() {
    let root = test_root("interrupted-stock-migration");
    fs::create_dir_all(&root).expect("theme root");
    let target = root.join("panel.css");
    fs::write(&target, OLD_STOCK).expect("legacy stock");
    let digest = blake3::hash(OLD_STOCK).to_hex().to_string();

    let result = migrate_stock_file_with_writer(
        &target,
        CURRENT_STOCK,
        &digest,
        BACKUP_TAG,
        |_path, _contents| {
            Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "test interruption",
            ))
        },
    );

    assert!(result.is_err());
    assert_eq!(fs::read(&target).expect("legacy stock"), OLD_STOCK);
    let backup = stock_backup_path(&target, BACKUP_TAG).expect("backup path");
    assert_eq!(fs::read(backup).expect("stock backup"), OLD_STOCK);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn matching_existing_backup_allows_a_retried_migration() {
    let root = test_root("stock-migration-retry");
    fs::create_dir_all(&root).expect("theme root");
    let target = root.join("panel.css");
    let backup = stock_backup_path(&target, BACKUP_TAG).expect("backup path");
    fs::write(&target, OLD_STOCK).expect("legacy stock");
    fs::write(&backup, OLD_STOCK).expect("matching stock backup");

    let migrated = migrate(&target, OLD_STOCK).expect("retried stock migration");

    assert!(migrated);
    assert_eq!(fs::read(&target).expect("current stock"), CURRENT_STOCK);
    assert_eq!(fs::read(&backup).expect("stock backup"), OLD_STOCK);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn conflicting_existing_backup_uses_a_new_suffix_without_overwriting() {
    let root = test_root("stock-migration-backup-conflict");
    fs::create_dir_all(&root).expect("theme root");
    let target = root.join("panel.css");
    let backup = stock_backup_path(&target, BACKUP_TAG).expect("backup path");
    fs::write(&target, OLD_STOCK).expect("legacy stock");
    fs::write(&backup, b"custom backup").expect("conflicting stock backup");

    let migrated = migrate(&target, OLD_STOCK).expect("collision-safe migration");
    let mut fallback_name = backup.as_os_str().to_os_string();
    fallback_name.push(".1");
    let fallback = std::path::PathBuf::from(fallback_name);

    assert!(migrated);
    assert_eq!(fs::read(&target).expect("current stock"), CURRENT_STOCK);
    assert_eq!(
        fs::read(&backup).expect("conflicting stock backup"),
        b"custom backup"
    );
    assert_eq!(
        fs::read(fallback).expect("fallback stock backup"),
        OLD_STOCK
    );
    let _ = fs::remove_dir_all(root);
}

fn migrate(target: &std::path::Path, legacy: &[u8]) -> Result<bool, super::super::ConfigError> {
    let digest = blake3::hash(legacy).to_hex().to_string();
    migrate_known_stock_file(target, CURRENT_STOCK, &digest, BACKUP_TAG)
}
