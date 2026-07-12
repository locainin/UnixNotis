use super::super::write::{atomic_temp_path, write_atomic};
use std::fs;
use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::fs::symlink;

#[cfg(unix)]
#[test]
fn write_atomic_bypasses_preexisting_temp_symlink_without_touching_it() {
    let root = test_root("backup-atomic-temp-symlink");
    let target = root.join("config.toml");
    let protected = root.join("protected");
    let temp_path = atomic_temp_path(&target);
    fs::write(&target, "old").expect("target");
    fs::write(&protected, "protected").expect("protected");
    symlink(&protected, &temp_path).expect("temp symlink");

    write_atomic(&target, "new").expect("alternate temp path");

    assert_eq!(fs::read_to_string(&target).expect("target updated"), "new");
    assert_eq!(
        fs::read_to_string(&protected).expect("protected remains"),
        "protected"
    );
    assert!(fs::symlink_metadata(&temp_path)
        .expect("temp remains")
        .file_type()
        .is_symlink());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn write_atomic_bypasses_stale_temp_regular_file() {
    let root = test_root("backup-atomic-temp-regular");
    let target = root.join("config.toml");
    let temp_path = atomic_temp_path(&target);
    fs::write(&target, "old").expect("target");
    fs::write(&temp_path, "stale").expect("stale temp");

    write_atomic(&target, "new").expect("alternate temp path");

    assert_eq!(fs::read_to_string(&target).expect("target updated"), "new");
    assert_eq!(
        fs::read_to_string(&temp_path).expect("temp remains"),
        "stale"
    );
    let _ = fs::remove_dir_all(root);
}

fn test_root(name: &str) -> PathBuf {
    // Target-local roots keep symlink tests contained inside the repository build directory
    let root =
        PathBuf::from("target").join(format!("unixnotis-installer-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("test root");
    root
}
