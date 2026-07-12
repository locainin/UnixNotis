use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{write_text_preserving_mode, write_text_with_mode};

fn test_root(label: &str) -> std::path::PathBuf {
    static NEXT_ROOT: AtomicUsize = AtomicUsize::new(0);
    let sequence = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "unixnotis-safe-write-{label}-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create test root");
    root
}

#[test]
fn secure_write_rejects_symlinked_ancestor_without_touching_target() {
    let root = test_root("ancestor-symlink");
    let real_parent = root.join("real");
    let linked_parent = root.join("linked");
    fs::create_dir_all(&real_parent).expect("create real parent");
    symlink(&real_parent, &linked_parent).expect("create parent symlink");

    let error = write_text_with_mode(&linked_parent.join("config.toml"), "unsafe", 0o644)
        .expect_err("reject symlinked ancestor");

    assert_eq!(
        error.raw_os_error(),
        Some(rustix::io::Errno::LOOP.raw_os_error())
    );
    assert!(!real_parent.join("config.toml").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn secure_write_preserves_existing_mode_and_replaces_contents() {
    let root = test_root("preserve-mode");
    let target = root.join("config.toml");
    fs::write(&target, "old").expect("write original");
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).expect("set mode");

    write_text_preserving_mode(&target, "new", 0o644).expect("secure replace");

    assert_eq!(fs::read_to_string(&target).expect("read target"), "new");
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
