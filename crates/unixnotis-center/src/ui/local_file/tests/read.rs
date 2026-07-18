use std::fs;
use std::io::Cursor;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{read_bytes_limited, read_regular_file, validate_file_size};

#[test]
fn regular_file_reader_returns_complete_bytes_from_one_descriptor() {
    let root = test_root("regular");
    let path = root.join("image.bin");
    fs::write(&path, b"image bytes").expect("write local file");

    assert_eq!(
        read_regular_file(&path, 64).expect("read regular file"),
        b"image bytes"
    );

    fs::remove_dir_all(root).expect("remove local file test root");
}

#[test]
fn regular_file_reader_rejects_oversized_and_non_regular_inputs() {
    let root = test_root("policy");
    let oversized = root.join("oversized.bin");
    fs::File::create(&oversized)
        .and_then(|file| file.set_len(65))
        .expect("create sparse oversized file");

    assert_eq!(
        read_regular_file(&oversized, 64)
            .expect_err("oversized local file must fail")
            .kind(),
        std::io::ErrorKind::InvalidData
    );
    assert_eq!(
        read_regular_file(&root, 64)
            .expect_err("directory must fail")
            .kind(),
        std::io::ErrorKind::InvalidInput
    );

    fs::remove_dir_all(root).expect("remove local file test root");
}

#[test]
fn regular_file_size_policy_accepts_the_exact_limit_only() {
    assert!(validate_file_size(64, 64).is_ok());
    assert_eq!(
        validate_file_size(65, 64)
            .expect_err("size above limit must fail")
            .kind(),
        std::io::ErrorKind::InvalidData
    );
}

#[test]
fn regular_file_reader_detects_growth_after_metadata_snapshot() {
    let mut exact = Cursor::new(b"1234");
    assert_eq!(
        read_bytes_limited(&mut exact, 4, 4).expect("exact limit should read"),
        b"1234"
    );

    let mut oversized = Cursor::new(b"12345");
    assert_eq!(
        read_bytes_limited(&mut oversized, 4, 4)
            .expect_err("growth beyond the snapshot must fail")
            .kind(),
        std::io::ErrorKind::InvalidData
    );
}

#[cfg(unix)]
#[test]
fn regular_file_reader_rejects_last_component_symlinks() {
    use std::os::unix::fs::symlink;

    let root = test_root("symlink");
    let target = root.join("target.bin");
    let link = root.join("link.bin");
    fs::write(&target, b"image bytes").expect("write symlink target");
    symlink(&target, &link).expect("create local file symlink");

    read_regular_file(&link, 64).expect_err("last-component symlink must fail");

    fs::remove_dir_all(root).expect("remove local file test root");
}

fn test_root(name: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "unixnotis-center-local-file-{name}-{}-{stamp}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create local file test root");
    root
}
