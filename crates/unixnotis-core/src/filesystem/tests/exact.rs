//! Exact regular-file transaction tests

use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};

use super::{
    ensure_exact_file, ensure_exact_file_pair, rollback_created_member, EnsureExactFileOutcome,
    EnsureExactFilePairOutcome, ExactMember,
};
use crate::filesystem::descriptor::open_parent_existing;
use crate::test_support::unique_temp_path;

#[test]
fn exact_file_creation_accepts_only_identical_existing_bytes() {
    let root = unique_temp_path("exact-file");
    fs::create_dir_all(&root).expect("create test root");
    let target = root.join("type");

    assert_eq!(
        ensure_exact_file(&target, b"bundle\n", 0o644).expect("create exact file"),
        EnsureExactFileOutcome::Created
    );
    assert_eq!(
        ensure_exact_file(&target, b"bundle\n", 0o600).expect("accept exact file"),
        EnsureExactFileOutcome::AlreadyExact
    );
    assert_eq!(
        ensure_exact_file(&target, b"longrun\n", 0o644).expect("reject mismatched bytes"),
        EnsureExactFileOutcome::ContentsMismatch
    );

    assert_eq!(
        fs::read_to_string(&target).expect("read exact file"),
        "bundle\n"
    );
    assert_eq!(
        fs::metadata(&target)
            .expect("exact file metadata")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn exact_file_creation_masks_non_permission_mode_bits() {
    let root = unique_temp_path("exact-file-mode-mask");
    fs::create_dir_all(&root).expect("create test root");
    let target = root.join("type");

    ensure_exact_file(&target, b"bundle\n", 0o100_600).expect("create exact file");

    assert_eq!(
        fs::metadata(&target)
            .expect("target metadata")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn exact_file_creation_never_follows_a_collision_symlink() {
    let root = unique_temp_path("exact-file-link");
    fs::create_dir_all(&root).expect("create test root");
    let outside = root.join("outside");
    let target = root.join("type");
    fs::write(&outside, "foreign").expect("write outside file");
    symlink(&outside, &target).expect("create exact-file link");

    ensure_exact_file(&target, b"bundle\n", 0o644).expect_err("link collision should fail");

    assert_eq!(
        fs::read_to_string(outside).expect("read outside file"),
        "foreign"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn exact_file_creation_preserves_non_directory_parent_errors() {
    let root = unique_temp_path("exact-file-blocked-parent");
    fs::create_dir_all(&root).expect("create test root");
    let parent_file = root.join("parent-file");
    fs::write(&parent_file, "not a directory").expect("write blocking parent");

    let error = ensure_exact_file(&parent_file.join("state"), b"data", 0o600)
        .expect_err("non-directory parent should fail");

    assert_eq!(error.kind(), std::io::ErrorKind::NotADirectory);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn exact_pair_creates_and_validates_both_members() {
    let root = unique_temp_path("exact-pair-create");
    fs::create_dir_all(&root).expect("create test root");
    let target = root.join("type");
    let marker = root.join(".created-type");

    assert_eq!(
        ensure_exact_file_pair(&target, b"bundle\n", 0o644, &marker, b"unixnotis\n", 0o600,)
            .expect("create exact pair"),
        EnsureExactFilePairOutcome::Created
    );
    assert_eq!(
        ensure_exact_file_pair(&target, b"bundle\n", 0o640, &marker, b"unixnotis\n", 0o644,)
            .expect("validate exact pair"),
        EnsureExactFilePairOutcome::AlreadyExact
    );
    assert_eq!(
        fs::metadata(&target)
            .expect("target metadata")
            .permissions()
            .mode()
            & 0o777,
        0o640
    );
    assert_eq!(
        fs::metadata(&marker)
            .expect("marker metadata")
            .permissions()
            .mode()
            & 0o777,
        0o644
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn exact_pair_preserves_an_unmarked_exact_existing_file() {
    let root = unique_temp_path("exact-pair-unmarked-file");
    fs::create_dir_all(&root).expect("create test root");
    let target = root.join("type");
    let marker = root.join(".created-type");
    fs::write(&target, b"bundle\n").expect("write exact existing file");

    let outcome =
        ensure_exact_file_pair(&target, b"bundle\n", 0o644, &marker, b"unixnotis\n", 0o644)
            .expect("preserve exact unmarked file");

    assert_eq!(outcome, EnsureExactFilePairOutcome::AlreadyExactUnowned);
    assert!(!marker.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn exact_pair_rejects_an_invalid_marker_shape_for_an_existing_file() {
    let root = unique_temp_path("exact-pair-invalid-marker");
    fs::create_dir_all(&root).expect("create test root");
    let target = root.join("type");
    let marker = root.join(".created-type");
    fs::write(&target, b"bundle\n").expect("write exact existing file");
    fs::create_dir(&marker).expect("create invalid marker directory");

    ensure_exact_file_pair(&target, b"bundle\n", 0o644, &marker, b"unixnotis\n", 0o644)
        .expect_err("an invalid marker shape must not be treated as missing");

    assert!(target.is_file());
    assert!(marker.is_dir());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn exact_pair_rolls_back_a_new_file_when_the_marker_conflicts() {
    let root = unique_temp_path("exact-pair-marker-conflict");
    fs::create_dir_all(&root).expect("create test root");
    let target = root.join("type");
    let marker = root.join(".created-type");
    fs::write(&marker, b"foreign\n").expect("write foreign marker");

    let outcome =
        ensure_exact_file_pair(&target, b"bundle\n", 0o644, &marker, b"unixnotis\n", 0o644)
            .expect("report marker conflict");

    assert_eq!(outcome, EnsureExactFilePairOutcome::ContentsMismatch);
    assert!(!target.exists());
    assert_eq!(
        fs::read_to_string(marker).expect("read foreign marker"),
        "foreign\n"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn exact_pair_preserves_an_existing_file_when_the_marker_conflicts() {
    let root = unique_temp_path("exact-pair-existing-file-conflict");
    fs::create_dir_all(&root).expect("create test root");
    let target = root.join("type");
    let marker = root.join(".created-type");
    fs::write(&target, b"bundle\n").expect("write exact existing file");
    fs::write(&marker, b"foreign\n").expect("write foreign marker");

    let outcome =
        ensure_exact_file_pair(&target, b"bundle\n", 0o644, &marker, b"unixnotis\n", 0o644)
            .expect("report marker conflict");

    assert_eq!(outcome, EnsureExactFilePairOutcome::ContentsMismatch);
    assert_eq!(
        fs::read_to_string(target).expect("read existing file"),
        "bundle\n"
    );
    assert_eq!(
        fs::read_to_string(marker).expect("read foreign marker"),
        "foreign\n"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn exact_pair_rejects_different_parents_and_reused_names() {
    let root = unique_temp_path("exact-pair-path-validation");
    fs::create_dir_all(root.join("other")).expect("create test roots");
    let target = root.join("type");

    ensure_exact_file_pair(
        &target,
        b"bundle\n",
        0o644,
        &root.join("other").join("marker"),
        b"unixnotis\n",
        0o644,
    )
    .expect_err("different parents should fail");
    ensure_exact_file_pair(&target, b"bundle\n", 0o644, &target, b"unixnotis\n", 0o644)
        .expect_err("reused names should fail");

    assert!(!target.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rollback_accepts_a_created_member_that_is_already_missing() {
    let root = unique_temp_path("exact-rollback-missing");
    fs::create_dir_all(&root).expect("create test root");
    let target = root.join("state");
    fs::write(&target, b"owned").expect("write target");
    let retained = fs::File::open(&target).expect("open retained target");
    let (parent_fd, file_name) = open_parent_existing(&target).expect("open retained parent");
    fs::remove_file(&target).expect("remove visible target");
    let member = ExactMember {
        file: retained,
        created: true,
    };

    rollback_created_member(&parent_fd, &file_name, &member)
        .expect("an already absent created member needs no rollback");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rollback_propagates_non_missing_lookup_errors() {
    let root = unique_temp_path("exact-rollback-lookup-error");
    fs::create_dir_all(&root).expect("create test root");
    let retained_path = root.join("retained");
    fs::write(&retained_path, b"owned").expect("write retained file");
    let member = ExactMember {
        file: fs::File::open(&retained_path).expect("open retained file"),
        created: true,
    };
    let (parent_fd, _file_name) =
        open_parent_existing(&retained_path).expect("open retained parent");
    let oversized_name = OsString::from("x".repeat(1_024));

    rollback_created_member(&parent_fd, &oversized_name, &member)
        .expect_err("invalid lookup errors must not be treated as a missing target");

    assert_eq!(
        fs::read_to_string(retained_path).expect("read retained file"),
        "owned"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rollback_preserves_a_same_device_replacement() {
    let root = unique_temp_path("exact-rollback-replacement");
    fs::create_dir_all(&root).expect("create test root");
    let target = root.join("state");
    let moved = root.join("original");
    fs::write(&target, b"owned").expect("write original target");
    let retained = fs::File::open(&target).expect("open retained target");
    let (parent_fd, file_name) = open_parent_existing(&target).expect("open retained parent");
    fs::rename(&target, &moved).expect("move original target");
    fs::write(&target, b"replacement").expect("write replacement target");
    let member = ExactMember {
        file: retained,
        created: true,
    };

    rollback_created_member(&parent_fd, &file_name, &member)
        .expect_err("identity mismatch must stop rollback");

    assert_eq!(
        fs::read_to_string(target).expect("read replacement target"),
        "replacement"
    );
    assert_eq!(
        fs::read_to_string(moved).expect("read original target"),
        "owned"
    );
    let _ = fs::remove_dir_all(root);
}
