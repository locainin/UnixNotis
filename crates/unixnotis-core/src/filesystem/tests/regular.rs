//! Stable regular-file operation tests

use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};

use rustix::fs::{mkfifoat, Mode, CWD};

use super::{make_file_executable, regular_file_contents_equal, set_file_mode};
use crate::test_support::unique_temp_path;

#[test]
fn bounded_comparison_accepts_exact_bytes_and_rejects_larger_files() {
    let root = unique_temp_path("regular-bounded-comparison");
    fs::create_dir_all(&root).expect("create test root");
    let target = root.join("service");
    fs::write(&target, b"bundle\n").expect("write exact file");

    assert!(
        regular_file_contents_equal(&target, b"bundle\n", 7).expect("compare exact regular file")
    );
    assert!(
        !regular_file_contents_equal(&target, b"bundle", 6).expect("reject oversized regular file")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn bounded_comparison_rejects_a_limit_smaller_than_expected_bytes() {
    let root = unique_temp_path("regular-invalid-comparison-limit");
    fs::create_dir_all(&root).expect("create test root");
    let target = root.join("service");
    fs::write(&target, b"bundle\n").expect("write exact file");

    let error = regular_file_contents_equal(&target, b"bundle\n", 6)
        .expect_err("invalid comparison limit should fail");

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn bounded_comparison_rejects_links_and_special_files_without_blocking() {
    let root = unique_temp_path("regular-unsafe-comparison");
    fs::create_dir_all(&root).expect("create test root");
    let outside = root.join("outside");
    let link = root.join("link");
    let fifo = root.join("fifo");
    fs::write(&outside, b"bundle\n").expect("write outside file");
    symlink(&outside, &link).expect("create comparison link");
    mkfifoat(CWD, &fifo, Mode::from_raw_mode(0o600)).expect("create comparison fifo");

    regular_file_contents_equal(&link, b"bundle\n", 7).expect_err("comparison link should fail");
    regular_file_contents_equal(&fifo, b"bundle\n", 7).expect_err("comparison fifo should fail");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn executable_update_rejects_symlink_without_touching_its_target() {
    let root = unique_temp_path("regular-executable-symlink");
    fs::create_dir_all(&root).expect("create test root");
    let outside = root.join("outside.sh");
    let link = root.join("script.sh");
    fs::write(&outside, "safe").expect("write outside script");
    fs::set_permissions(&outside, fs::Permissions::from_mode(0o600)).expect("set outside mode");
    symlink(&outside, &link).expect("create script link");

    make_file_executable(&link).expect_err("script link should fail");

    assert_eq!(fs::read_to_string(&outside).expect("read outside"), "safe");
    assert_eq!(
        fs::metadata(&outside)
            .expect("outside metadata")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn mode_update_applies_exact_permissions_to_a_regular_file() {
    let root = unique_temp_path("regular-mode-update");
    fs::create_dir_all(&root).expect("create test root");
    let target = root.join("run");
    fs::write(&target, "service").expect("write service file");
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).expect("set original mode");

    set_file_mode(&target, 0o755).expect("set service mode");

    assert_eq!(
        fs::metadata(&target)
            .expect("service metadata")
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn mode_update_rejects_a_symlink_without_touching_its_target() {
    let root = unique_temp_path("regular-mode-symlink");
    fs::create_dir_all(&root).expect("create test root");
    let outside = root.join("outside");
    let link = root.join("run");
    fs::write(&outside, "service").expect("write outside file");
    fs::set_permissions(&outside, fs::Permissions::from_mode(0o600)).expect("set outside mode");
    symlink(&outside, &link).expect("create mode link");

    set_file_mode(&link, 0o755).expect_err("service link should fail");

    assert_eq!(
        fs::metadata(&outside)
            .expect("outside metadata")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    let _ = fs::remove_dir_all(root);
}
