use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use rustix::fs::{mkfifoat, open, Mode, OFlags, CWD};

use super::{
    existing_mode_or_default, validate_target_at, write_text_preserving_mode, write_text_with_mode,
};

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

#[test]
fn metadata_validation_rejects_fifo_without_waiting_for_a_writer() {
    let root = test_root("fifo-target");
    let target = root.join("config.fifo");
    mkfifoat(CWD, &target, Mode::RUSR | Mode::WUSR).expect("create FIFO target");
    let worker_target = target.clone();
    let (result_tx, result_rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let result = write_text_preserving_mode(&worker_target, "new", 0o644);
        result_tx.send(result).expect("send FIFO validation result");
    });

    let result = match result_rx.recv_timeout(Duration::from_secs(2)) {
        Ok(result) => result,
        Err(error) => {
            // A writer releases a regressed read-only FIFO open before the test reports failure
            let _writer = open(
                &target,
                OFlags::WRONLY | OFlags::CLOEXEC | OFlags::NONBLOCK,
                Mode::empty(),
            )
            .expect("unblock FIFO reader");
            let _ = result_rx.recv_timeout(Duration::from_secs(2));
            worker.join().expect("join unblocked FIFO worker");
            panic!("FIFO validation exceeded its focused deadline: {error}");
        }
    };
    worker.join().expect("join FIFO validation worker");

    assert!(result.expect_err("reject FIFO target").kind() == std::io::ErrorKind::InvalidInput);
    fs::remove_dir_all(root).expect("remove FIFO test root");
}

#[test]
fn metadata_validation_rejects_socket_device_and_final_symlink_targets() {
    let root = test_root("special-targets");
    let socket = root.join("installer.sock");
    let _listener = UnixListener::bind(&socket).expect("bind socket target");
    let sentinel = root.join("sentinel.txt");
    let link = root.join("linked.txt");
    fs::write(&sentinel, "sentinel").expect("write sentinel");
    symlink(&sentinel, &link).expect("create final symlink");

    for target in [&socket, Path::new("/dev/null"), &link] {
        assert!(
            write_text_with_mode(target, "new", 0o644).is_err(),
            "special target should be rejected: {}",
            target.display()
        );
    }
    assert_eq!(
        fs::read_to_string(&sentinel).expect("read sentinel"),
        "sentinel"
    );
    fs::remove_dir_all(root).expect("remove special target test root");
}

#[test]
fn metadata_validation_does_not_treat_other_open_errors_as_missing_files() {
    let root = test_root("metadata-open-errors");
    let overlong_name = "x".repeat(300);

    assert!(existing_mode_or_default(&root.join(&overlong_name), 0o644).is_err());

    let parent_fd = open(&root, OFlags::DIRECTORY | OFlags::CLOEXEC, Mode::empty())
        .expect("open validation parent");
    assert!(validate_target_at(&parent_fd, &overlong_name).is_err());

    fs::remove_dir_all(root).expect("remove metadata error test root");
}
