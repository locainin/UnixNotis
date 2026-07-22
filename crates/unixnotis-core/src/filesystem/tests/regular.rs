//! Stable regular-file operation tests

use std::fs;
use std::io::Read;
use std::os::unix::fs::{symlink, PermissionsExt};

use rustix::fs::{mkfifoat, Mode, CWD};

use super::{
    make_file_executable, open_regular_file, read_regular_file_bounded,
    regular_file_contents_equal, set_file_mode,
};
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

#[test]
fn bounded_regular_file_read_accepts_the_exact_limit() {
    let root = unique_temp_path("read-regular-exact-limit");
    let path = root.join("style.css");
    fs::create_dir_all(&root).expect("create root");
    fs::write(&path, b"12345678").expect("write file");

    let contents = read_regular_file_bounded(&path, 8).expect("read bounded file");

    assert_eq!(contents, b"12345678");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn bounded_regular_file_read_rejects_a_file_over_the_limit() {
    let root = unique_temp_path("read-regular-over-limit");
    let path = root.join("style.css");
    fs::create_dir_all(&root).expect("create root");
    fs::write(&path, b"123456789").expect("write file");

    let error = read_regular_file_bounded(&path, 8).expect_err("oversized file should fail");

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn bounded_regular_file_read_rejects_a_source_symlink() {
    let root = unique_temp_path("read-regular-symlink");
    let protected = root.join("protected.css");
    let path = root.join("style.css");
    fs::create_dir_all(&root).expect("create root");
    fs::write(&protected, "protected").expect("write protected file");
    symlink(&protected, &path).expect("create file link");

    read_regular_file_bounded(&path, 1024).expect_err("source link should fail");

    assert_eq!(
        fs::read_to_string(protected).expect("read protected file"),
        "protected"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn bounded_regular_file_read_rejects_a_linked_parent() {
    let root = unique_temp_path("read-regular-linked-parent");
    let outside = root.join("outside");
    let linked = root.join("linked");
    fs::create_dir_all(&outside).expect("create outside directory");
    fs::write(outside.join("style.css"), "outside theme").expect("write outside file");
    symlink(&outside, &linked).expect("create parent link");

    read_regular_file_bounded(&linked.join("style.css"), 1024)
        .expect_err("linked parent should fail");

    assert_eq!(
        fs::read_to_string(outside.join("style.css")).expect("read outside file"),
        "outside theme"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn bounded_regular_file_read_rejects_a_directory() {
    let root = unique_temp_path("read-regular-directory");
    let path = root.join("style.css");
    fs::create_dir_all(&path).expect("create directory target");

    read_regular_file_bounded(&path, 1024).expect_err("directory should fail");

    assert!(path.is_dir());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn open_regular_file_retains_the_validated_object_after_path_replacement() {
    let root = unique_temp_path("open-regular-pinned");
    let path = root.join("sound.ogg");
    let moved = root.join("original.ogg");
    fs::create_dir_all(&root).expect("create root");
    fs::write(&path, b"original").expect("write original file");
    let mut file = open_regular_file(&path).expect("open validated file");

    fs::rename(&path, &moved).expect("move original file");
    fs::write(&path, b"replacement").expect("write replacement file");
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .expect("read retained descriptor");

    assert_eq!(contents, "original");
    assert_eq!(
        fs::read_to_string(path).expect("read replacement"),
        "replacement"
    );
    let _ = fs::remove_dir_all(root);
}
