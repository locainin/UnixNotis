//! Private quarantine lifecycle tests

use std::fs;

use super::Quarantine;
use crate::filesystem::descriptor::open_parent_existing;
use crate::test_support::unique_temp_path;

#[test]
fn quarantine_moves_and_restores_an_entry_without_following_a_new_source_name() {
    let root = unique_temp_path("quarantine-restore");
    let source = root.join("state.json");
    fs::create_dir_all(&root).expect("create root");
    fs::write(&source, "original").expect("write original");

    let (parent_fd, source_name) = open_parent_existing(&source).expect("open source parent");
    let quarantine = Quarantine::create(&parent_fd).expect("create quarantine");
    let entry = quarantine
        .move_entry(&parent_fd, &source_name)
        .expect("move source into quarantine");
    fs::write(&source, "replacement").expect("write replacement source");

    quarantine
        .restore(&entry, &parent_fd, &source_name)
        .expect_err("restore must not replace an unrelated destination");
    fs::remove_file(&source).expect("remove test replacement");
    quarantine
        .restore(&entry, &parent_fd, &source_name)
        .expect("restore original source name");

    assert_eq!(
        fs::read_to_string(&source).expect("read restored source"),
        "original"
    );
    quarantine
        .cleanup(&parent_fd)
        .expect("remove empty quarantine");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn quarantine_keeps_an_entry_when_cleanup_is_not_requested() {
    let root = unique_temp_path("quarantine-retain");
    let source = root.join("state.json");
    fs::create_dir_all(&root).expect("create root");
    fs::write(&source, "original").expect("write original");

    let (parent_fd, source_name) = open_parent_existing(&source).expect("open source parent");
    let quarantine = Quarantine::create(&parent_fd).expect("create quarantine");
    let entry = quarantine
        .move_entry(&parent_fd, &source_name)
        .expect("move source into quarantine");
    quarantine
        .unlink(&entry)
        .expect("unlink quarantined source");
    quarantine
        .cleanup(&parent_fd)
        .expect("remove empty quarantine");

    assert!(!source.exists());
    let _ = fs::remove_dir_all(root);
}
