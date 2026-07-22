//! Write-target preflight tests

use std::fs;
use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::reject_unsafe_write_target;

fn test_root(label: &str) -> PathBuf {
    static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);
    let sequence = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
    let root = PathBuf::from("target").join(format!(
        "unixnotis-write-target-{label}-{}-{sequence}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create test root");
    root
}

#[test]
fn regular_and_missing_write_targets_are_accepted() {
    let root = test_root("accepted");
    let regular = root.join("config.toml");
    fs::write(&regular, "config").expect("write regular target");

    reject_unsafe_write_target(&regular).expect("regular file should be accepted");
    reject_unsafe_write_target(&root.join("missing.toml"))
        .expect("missing file should be accepted");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn symlink_and_non_file_write_targets_are_rejected() {
    let root = test_root("rejected");
    let regular = root.join("config.toml");
    let link = root.join("linked.toml");
    let directory = root.join("directory.toml");
    fs::write(&regular, "config").expect("write regular target");
    symlink(&regular, &link).expect("create target symlink");
    fs::create_dir(&directory).expect("create directory target");

    assert_eq!(
        reject_unsafe_write_target(&link)
            .expect_err("target symlink should fail")
            .kind(),
        std::io::ErrorKind::InvalidInput
    );
    assert_eq!(
        reject_unsafe_write_target(&directory)
            .expect_err("directory target should fail")
            .kind(),
        std::io::ErrorKind::InvalidInput
    );

    let _ = fs::remove_dir_all(root);
}
