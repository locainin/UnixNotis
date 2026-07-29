//! No-replace regular-file rename tests

use std::fs;
use std::os::unix::fs::symlink;

use super::{
    classify_directory_rename_attempt, classify_rename_attempt, rename_directory_no_replace,
    rename_regular_file_no_replace, RenameDirectoryOutcome, RenameRegularFileOutcome,
};
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
fn rename_attempt_result_distinguishes_every_kernel_outcome() {
    assert_eq!(
        classify_rename_attempt(Ok(())).expect("successful rename"),
        RenameRegularFileOutcome::Renamed
    );
    assert_eq!(
        classify_rename_attempt(Err(std::io::ErrorKind::AlreadyExists.into()))
            .expect("destination collision"),
        RenameRegularFileOutcome::DestinationExists
    );
    assert_eq!(
        classify_rename_attempt(Err(std::io::ErrorKind::NotFound.into()))
            .expect("source disappeared"),
        RenameRegularFileOutcome::SourceMissing
    );

    let error = classify_rename_attempt(Err(std::io::ErrorKind::PermissionDenied.into()))
        .expect_err("unrelated rename failure should propagate");
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
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

#[test]
fn directory_rename_publishes_a_complete_tree_without_replacing_a_destination() {
    let root = unique_temp_path("rename-directory");
    let source = root.join(".stock.staging");
    let destination = root.join("stock");
    fs::create_dir_all(&source).expect("create staged directory");
    fs::write(source.join("theme.toml"), "api_version = 2").expect("write staged manifest");

    let outcome =
        rename_directory_no_replace(&source, &destination).expect("publish staged directory");

    assert_eq!(outcome, RenameDirectoryOutcome::Renamed);
    assert!(!source.exists());
    assert_eq!(
        fs::read_to_string(destination.join("theme.toml")).expect("read published manifest"),
        "api_version = 2"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn directory_rename_preserves_an_existing_destination_and_staged_source() {
    let root = unique_temp_path("rename-directory-collision");
    let source = root.join(".stock.staging");
    let destination = root.join("stock");
    fs::create_dir_all(&source).expect("create staged directory");
    fs::create_dir_all(&destination).expect("create destination directory");
    fs::write(source.join("staged.css"), "staged").expect("write staged file");
    fs::write(destination.join("personal.css"), "personal").expect("write personal file");

    let outcome =
        rename_directory_no_replace(&source, &destination).expect("classify destination collision");

    assert_eq!(outcome, RenameDirectoryOutcome::DestinationExists);
    assert_eq!(
        fs::read_to_string(source.join("staged.css")).expect("read retained staged file"),
        "staged"
    );
    assert_eq!(
        fs::read_to_string(destination.join("personal.css")).expect("read personal file"),
        "personal"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn directory_rename_rejects_a_symlink_source() {
    let root = unique_temp_path("rename-directory-symlink");
    let actual = root.join("actual");
    let source = root.join(".stock.staging");
    let destination = root.join("stock");
    fs::create_dir_all(&actual).expect("create actual directory");
    symlink(&actual, &source).expect("create staged directory link");

    rename_directory_no_replace(&source, &destination)
        .expect_err("a staged directory link must be rejected");

    assert!(actual.is_dir());
    assert!(!destination.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn directory_rename_reports_a_missing_staged_source_without_creating_a_destination() {
    let root = unique_temp_path("rename-directory-missing-source");
    let source = root.join(".stock.staging");
    let destination = root.join("stock");
    fs::create_dir_all(&root).expect("create rename test root");

    let outcome = rename_directory_no_replace(&source, &destination)
        .expect("a missing staged directory should be a normal classified outcome");

    assert_eq!(outcome, RenameDirectoryOutcome::SourceMissing);
    assert!(!destination.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn directory_rename_classifies_source_disappearance_at_the_rename_boundary() {
    let outcome =
        classify_directory_rename_attempt(Err(std::io::Error::from(std::io::ErrorKind::NotFound)))
            .expect("rename-time source disappearance should be classified");

    assert_eq!(outcome, RenameDirectoryOutcome::SourceMissing);
}
