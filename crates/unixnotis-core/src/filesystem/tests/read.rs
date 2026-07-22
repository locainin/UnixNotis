//! Bounded regular-file read tests

use std::fs;
use std::io::Read;
use std::os::unix::fs::symlink;

use super::{open_regular_file, read_regular_file_bounded};
use crate::test_support::unique_temp_path;

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
