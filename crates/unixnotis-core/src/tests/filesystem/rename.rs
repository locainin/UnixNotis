use std::fs;
use std::os::unix::fs::symlink;

use super::{rename_regular_file_no_replace, RenameRegularFileOutcome};
use crate::test_support::unique_temp_path;

#[test]
fn regular_file_rename_moves_source_to_an_unused_destination() {
    let root = unique_temp_path("rename-regular-file");
    let source = root.join("style.css");
    let destination = root.join("style.css.bak");
    fs::create_dir_all(&root).expect("create root");
    fs::write(&source, "legacy theme").expect("write source");

    let outcome = rename_regular_file_no_replace(&source, &destination).expect("rename file");

    assert_eq!(outcome, RenameRegularFileOutcome::Renamed);
    assert!(!source.exists());
    assert_eq!(
        fs::read_to_string(destination).expect("read destination"),
        "legacy theme"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn regular_file_rename_reports_a_missing_source_without_creating_parents() {
    let root = unique_temp_path("rename-missing-source");
    let source = root.join("missing").join("style.css");
    let destination = root.join("backup").join("style.css.bak");

    let outcome =
        rename_regular_file_no_replace(&source, &destination).expect("missing source outcome");

    assert_eq!(outcome, RenameRegularFileOutcome::SourceMissing);
    assert!(!root.exists());
}

#[test]
fn regular_file_rename_reports_a_missing_final_source() {
    let root = unique_temp_path("rename-missing-final-source");
    let source = root.join("style.css");
    let destination = root.join("style.css.bak");
    fs::create_dir_all(&root).expect("create root");

    let outcome =
        rename_regular_file_no_replace(&source, &destination).expect("missing source outcome");

    assert_eq!(outcome, RenameRegularFileOutcome::SourceMissing);
    assert!(!destination.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn regular_file_rename_propagates_a_non_collision_destination_error() {
    let root = unique_temp_path("rename-invalid-destination");
    let source = root.join("style.css");
    fs::create_dir_all(&root).expect("create root");
    fs::write(&source, "legacy theme").expect("write source");
    let destination = root.join("x".repeat(300));

    let error = rename_regular_file_no_replace(&source, &destination)
        .expect_err("overlong destination should fail");

    assert_ne!(error.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(
        fs::read_to_string(&source).expect("read source"),
        "legacy theme"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn regular_file_rename_preserves_an_existing_destination() {
    let root = unique_temp_path("rename-existing-destination");
    let source = root.join("style.css");
    let destination = root.join("style.css.bak");
    fs::create_dir_all(&root).expect("create root");
    fs::write(&source, "legacy theme").expect("write source");
    fs::write(&destination, "existing backup").expect("write destination");

    let outcome =
        rename_regular_file_no_replace(&source, &destination).expect("preserve destination");

    assert_eq!(outcome, RenameRegularFileOutcome::DestinationExists);
    assert_eq!(
        fs::read_to_string(source).expect("read source"),
        "legacy theme"
    );
    assert_eq!(
        fs::read_to_string(destination).expect("read destination"),
        "existing backup"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn regular_file_rename_rejects_a_source_symlink() {
    let root = unique_temp_path("rename-source-symlink");
    let protected = root.join("protected.css");
    let source = root.join("style.css");
    let destination = root.join("style.css.bak");
    fs::create_dir_all(&root).expect("create root");
    fs::write(&protected, "protected").expect("write protected file");
    symlink(&protected, &source).expect("create source link");

    rename_regular_file_no_replace(&source, &destination)
        .expect_err("source link should be rejected");

    assert!(fs::symlink_metadata(source)
        .expect("source link remains")
        .file_type()
        .is_symlink());
    assert_eq!(
        fs::read_to_string(protected).expect("read protected file"),
        "protected"
    );
    assert!(!destination.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn regular_file_rename_rejects_a_linked_parent() {
    let root = unique_temp_path("rename-linked-parent");
    let outside = root.join("outside");
    let linked = root.join("linked");
    let source = linked.join("style.css");
    let destination = linked.join("style.css.bak");
    fs::create_dir_all(&outside).expect("create outside directory");
    fs::write(outside.join("style.css"), "outside theme").expect("write outside source");
    symlink(&outside, &linked).expect("create parent link");

    rename_regular_file_no_replace(&source, &destination)
        .expect_err("linked parent should be rejected");

    assert_eq!(
        fs::read_to_string(outside.join("style.css")).expect("read outside source"),
        "outside theme"
    );
    assert!(!outside.join("style.css.bak").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn regular_file_rename_rejects_a_directory_source() {
    let root = unique_temp_path("rename-directory-source");
    let source = root.join("style.css");
    let destination = root.join("style.css.bak");
    fs::create_dir_all(&source).expect("create source directory");

    rename_regular_file_no_replace(&source, &destination)
        .expect_err("directory source should be rejected");

    assert!(source.is_dir());
    assert!(!destination.exists());
    let _ = fs::remove_dir_all(root);
}
