//! Tests for safe configuration file writes

use std::fs;

use super::support::test_root;

#[test]
fn write_if_missing_preserves_existing_contents() {
    let root = test_root("write-if-missing");
    // Existing files should be treated as user-owned content
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("root");
    let path = root.join("file.txt");
    fs::write(&path, "keep").expect("existing file");

    super::super::write::write_if_missing(&path, "replace").expect("write should succeed");

    assert_eq!(fs::read_to_string(&path).expect("file contents"), "keep");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn write_if_missing_creates_new_file() {
    let root = test_root("write-if-missing-create");
    // Missing files are safe for bootstrap helpers to create
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("root");
    let path = root.join("file.txt");

    super::super::write::write_if_missing(&path, "created").expect("write should succeed");

    assert_eq!(fs::read_to_string(&path).expect("file contents"), "created");

    let _ = fs::remove_dir_all(root);
}
