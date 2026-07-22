//! Descriptor-relative removal tests

use std::fs;
use std::os::unix::fs::symlink;

use rustix::fs::{mkfifoat, Mode, CWD};

use super::{
    existing_parent, file_lookup_is_missing, remove_regular_file,
    remove_regular_file_pair_if_contents, remove_symlink, remove_symlink_if_target,
    revalidate_file_identity, RemoveExactFileOutcome, RemoveSymlinkOutcome,
};
use crate::filesystem::regular::open_regular_file_at;
use crate::filesystem::symlink::read_symlink;
use crate::test_support::unique_temp_path;

#[test]
fn optional_file_lookup_classifies_only_missing_errors() {
    assert!(file_lookup_is_missing(&std::io::ErrorKind::NotFound.into()));
    assert!(!file_lookup_is_missing(
        &std::io::ErrorKind::PermissionDenied.into()
    ));
    assert!(!file_lookup_is_missing(
        &std::io::ErrorKind::InvalidInput.into()
    ));
}

#[test]
fn retained_file_identity_rejects_a_same_directory_replacement() {
    let root = unique_temp_path("remove-file-identity");
    fs::create_dir_all(&root).expect("create root");
    let target = root.join("shared");
    let moved = root.join("original");
    fs::write(&target, "original").expect("write original");
    let (parent_fd, file_name) = existing_parent(&target)
        .expect("open parent")
        .expect("parent exists");
    let retained = open_regular_file_at(&parent_fd, &file_name).expect("open retained file");

    revalidate_file_identity(&parent_fd, &file_name, &retained)
        .expect("unchanged file should pass");
    fs::rename(&target, &moved).expect("move original");
    fs::write(&target, "replacement").expect("write replacement");

    revalidate_file_identity(&parent_fd, &file_name, &retained)
        .expect_err("replacement identity must fail");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn regular_file_removal_is_idempotent() {
    let root = unique_temp_path("remove-regular-file");
    let target = root.join("state.json");
    fs::create_dir_all(&root).expect("create root");
    fs::write(&target, "state").expect("write target");

    assert!(remove_regular_file(&target).expect("remove regular file"));
    assert!(!remove_regular_file(&target).expect("missing file stays removed"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn regular_file_removal_rejects_a_symlink_and_keeps_its_target() {
    let root = unique_temp_path("remove-regular-symlink");
    let protected = root.join("protected");
    let link = root.join("state.json");
    fs::create_dir_all(&root).expect("create root");
    fs::write(&protected, "protected").expect("write protected");
    symlink(&protected, &link).expect("create link");

    remove_regular_file(&link).expect_err("regular removal should reject a link");

    assert_eq!(
        fs::read_to_string(protected).expect("read protected"),
        "protected"
    );
    assert!(fs::symlink_metadata(link)
        .expect("link remains")
        .file_type()
        .is_symlink());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn regular_file_removal_rejects_a_symlinked_parent() {
    let root = unique_temp_path("remove-regular-parent-symlink");
    let outside = root.join("outside");
    let linked_parent = root.join("linked");
    fs::create_dir_all(&outside).expect("create outside");
    fs::write(outside.join("state.json"), "state").expect("write outside state");
    symlink(&outside, &linked_parent).expect("create parent link");

    remove_regular_file(&linked_parent.join("state.json")).expect_err("linked parent should fail");

    assert_eq!(
        fs::read_to_string(outside.join("state.json")).expect("read outside state"),
        "state"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn exact_pair_removal_requires_both_payloads_before_unlinking_either_file() {
    let root = unique_temp_path("remove-exact-pair");
    let target = root.join("type");
    let marker = root.join(".owner");
    fs::create_dir_all(&root).expect("create test root");
    fs::write(&target, "bundle\n").expect("write shared file");
    fs::write(&marker, "foreign\n").expect("write foreign marker");

    assert_eq!(
        remove_regular_file_pair_if_contents(&target, b"bundle\n", &marker, b"owned\n")
            .expect("inspect exact pair"),
        RemoveExactFileOutcome::ContentsMismatch
    );
    assert!(target.exists());
    assert!(marker.exists());

    fs::write(&marker, "owned\n").expect("repair marker");
    assert_eq!(
        remove_regular_file_pair_if_contents(&target, b"bundle\n", &marker, b"owned\n")
            .expect("remove exact pair"),
        RemoveExactFileOutcome::Removed
    );
    assert!(!target.exists());
    assert!(!marker.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn exact_pair_removal_reports_each_missing_member() {
    let root = unique_temp_path("remove-exact-missing-member");
    let target = root.join("type");
    let marker = root.join(".owner");
    fs::create_dir_all(&root).expect("create test root");

    assert_eq!(
        remove_regular_file_pair_if_contents(&target, b"bundle\n", &marker, b"owned\n")
            .expect("both missing is idempotent"),
        RemoveExactFileOutcome::Missing
    );
    fs::write(&target, "bundle\n").expect("write shared file");
    assert_eq!(
        remove_regular_file_pair_if_contents(&target, b"bundle\n", &marker, b"owned\n")
            .expect("missing marker is idempotent"),
        RemoveExactFileOutcome::Missing
    );
    fs::remove_file(&target).expect("remove shared file");
    fs::write(&marker, "owned\n").expect("write marker");
    assert_eq!(
        remove_regular_file_pair_if_contents(&target, b"bundle\n", &marker, b"owned\n")
            .expect("missing shared file is idempotent"),
        RemoveExactFileOutcome::Missing
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn exact_pair_removal_rejects_special_objects_for_each_member() {
    let root = unique_temp_path("remove-exact-special-member");
    let target = root.join("type");
    let marker = root.join(".owner");
    fs::create_dir_all(&root).expect("create test root");

    fs::create_dir(&target).expect("create target directory");
    fs::write(&marker, "owned\n").expect("write marker");
    remove_regular_file_pair_if_contents(&target, b"bundle\n", &marker, b"owned\n")
        .expect_err("target directory must be rejected");
    fs::remove_dir(&target).expect("remove target directory");

    fs::write(&target, "bundle\n").expect("write target");
    fs::remove_file(&marker).expect("remove marker");
    mkfifoat(CWD, &marker, Mode::from_raw_mode(0o600)).expect("create marker FIFO");
    remove_regular_file_pair_if_contents(&target, b"bundle\n", &marker, b"owned\n")
        .expect_err("marker FIFO must be rejected");

    assert_eq!(
        fs::read_to_string(&target).expect("shared file remains"),
        "bundle\n"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn symlink_removal_keeps_the_link_target() {
    let root = unique_temp_path("remove-symlink");
    let target = root.join("service");
    let link = root.join("enabled");
    fs::create_dir_all(&root).expect("create root");
    fs::write(&target, "service").expect("write target");
    symlink(&target, &link).expect("create link");

    assert!(remove_symlink(&link).expect("remove link"));
    assert!(!remove_symlink(&link).expect("missing link stays removed"));

    assert_eq!(fs::read_to_string(target).expect("read target"), "service");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn target_checked_symlink_removal_reports_mismatch_without_removing_link() {
    let root = unique_temp_path("remove-symlink-mismatch");
    let link = root.join("enabled");
    fs::create_dir_all(&root).expect("create root");
    symlink("actual", &link).expect("create link");

    let outcome = remove_symlink_if_target(&link, std::path::Path::new("expected"))
        .expect("inspect link target");

    assert_eq!(
        outcome,
        RemoveSymlinkOutcome::TargetMismatch("actual".into())
    );
    assert_eq!(
        read_symlink(&link).expect("read link"),
        Some("actual".into())
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn target_checked_symlink_removal_removes_only_an_exact_match() {
    let root = unique_temp_path("remove-symlink-match");
    let link = root.join("enabled");
    fs::create_dir_all(&root).expect("create root");
    symlink("../service", &link).expect("create link");

    let outcome = remove_symlink_if_target(&link, std::path::Path::new("../service"))
        .expect("remove matching link");

    assert_eq!(outcome, RemoveSymlinkOutcome::Removed);
    assert_eq!(read_symlink(&link).expect("link is missing"), None);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn target_checked_symlink_removal_reports_a_missing_final_entry() {
    let root = unique_temp_path("remove-symlink-missing-final");
    fs::create_dir_all(&root).expect("create root");

    let outcome = remove_symlink_if_target(&root.join("missing"), std::path::Path::new("service"))
        .expect("missing final link should be idempotent");

    assert_eq!(outcome, RemoveSymlinkOutcome::Missing);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn symlink_operations_reject_regular_files() {
    let root = unique_temp_path("remove-symlink-regular");
    let target = root.join("enabled");
    fs::create_dir_all(&root).expect("create root");
    fs::write(&target, "regular").expect("write regular file");

    read_symlink(&target).expect_err("read should reject a regular file");
    remove_symlink(&target).expect_err("removal should reject a regular file");
    remove_symlink_if_target(&target, std::path::Path::new("service"))
        .expect_err("target-checked removal should reject a regular file");

    assert_eq!(
        fs::read_to_string(target).expect("read regular file"),
        "regular"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn removal_does_not_create_a_missing_parent() {
    let root = unique_temp_path("remove-missing-parent");
    let missing_parent = root.join("missing");
    let target = missing_parent.join("state.json");

    assert!(!remove_regular_file(&target).expect("regular file is missing"));
    assert!(!remove_symlink(&target).expect("link is missing"));
    assert_eq!(read_symlink(&target).expect("link is missing"), None);
    assert_eq!(
        remove_symlink_if_target(&target, std::path::Path::new("service"))
            .expect("link is missing"),
        RemoveSymlinkOutcome::Missing
    );

    assert!(!missing_parent.exists());
    let _ = fs::remove_dir_all(root);
}
