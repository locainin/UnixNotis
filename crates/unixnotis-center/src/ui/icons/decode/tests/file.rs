use std::fs;

use super::super::file::{read_icon_file, MAX_ICON_BYTES};
use super::support::test_root;

#[test]
fn icon_file_read_returns_bytes_from_one_regular_descriptor() {
    let root = test_root("regular-read");
    let path = root.join("icon.png");
    fs::write(&path, b"icon bytes").expect("write icon");

    assert_eq!(read_icon_file(&path).expect("read icon"), b"icon bytes");

    fs::remove_dir_all(root).expect("remove icon test root");
}

#[test]
fn icon_file_read_rejects_oversized_and_non_regular_inputs() {
    let root = test_root("file-policy");
    let oversized = root.join("oversized.png");
    fs::File::create(&oversized)
        .and_then(|file| file.set_len(MAX_ICON_BYTES + 1))
        .expect("create sparse oversized icon");

    assert!(read_icon_file(&oversized)
        .expect_err("oversized icon must fail")
        .contains("byte limit"));
    assert!(read_icon_file(&root)
        .expect_err("directory must fail")
        .contains("regular file"));

    fs::remove_dir_all(root).expect("remove icon test root");
}

#[test]
fn icon_file_size_policy_accepts_the_exact_byte_limit_only() {
    assert_eq!(MAX_ICON_BYTES, 16 * 1_024 * 1_024);
}

#[cfg(unix)]
#[test]
fn icon_file_read_rejects_last_component_symlinks() {
    use std::os::unix::fs::symlink;

    let root = test_root("symlink");
    let target = root.join("target.png");
    let link = root.join("link.png");
    fs::write(&target, b"icon bytes").expect("write target");
    symlink(&target, &link).expect("create icon link");

    read_icon_file(&link).expect_err("last-component symlink must fail");

    fs::remove_dir_all(root).expect("remove icon test root");
}
