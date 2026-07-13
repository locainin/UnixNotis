use super::super::{
    create_backup_dir_secure, open_secure_dir_all, read_relative_file_secure,
    read_relative_file_secure_bounded, remove_empty_relative_dirs_secure,
    remove_relative_dir_secure, remove_relative_file_secure, write_relative_file_atomic_secure,
};
use crate::preset::filesystem::secure::{
    backup_name_is_taken, child_directory_is_missing, directory_open_flags,
    empty_dir_cleanup_is_complete, read_open_flags, secure_anchor_resolve_flags,
    secure_resolve_flags, temp_file_open_flags,
};
use rustix::fs::{OFlags, ResolveFlags};
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEST_TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(name: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock moved backwards")
            .as_nanos();
        let serial = TEST_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "unixnotis-preset-filesystem-secure-{name}-{stamp}-{serial}"
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    fn write(&self, relative_path: &str, contents: &str) {
        // Plain test writes keep the fixture setup simple and separate from the secure helpers
        let path = self.path.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        fs::write(path, contents).expect("write file");
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn secure_atomic_write_replaces_existing_file() {
    // Secure writes should keep the final file in place with new contents
    let root = TempDirGuard::new("atomic");
    let target = root.path.join("scripts/run.sh");
    root.write("scripts/run.sh", "old");

    let root_fd = open_secure_dir_all(&root.path).expect("open secure root");
    write_relative_file_atomic_secure(&root_fd, Path::new("scripts/run.sh"), b"new", 0o755)
        .expect("write file");

    assert_eq!(fs::read_to_string(&target).expect("read file"), "new");
    let mode = fs::metadata(&target)
        .expect("written file metadata")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o755, "requested mode should remain exact");
    assert_eq!(
        fs::read_dir(target.parent().expect("target parent"))
            .expect("read target parent")
            .count(),
        1,
        "atomic write should not leave a temporary file"
    );
}

#[test]
fn secure_atomic_write_cleans_temp_payload_when_rename_fails() {
    let root = TempDirGuard::new("rename-cleanup");
    fs::create_dir(root.path.join("target")).expect("create conflicting directory");
    let root_fd = open_secure_dir_all(&root.path).expect("open secure root");

    write_relative_file_atomic_secure(&root_fd, Path::new("target"), b"private", 0o600)
        .expect_err("file cannot replace a directory");

    let entries = fs::read_dir(&root.path)
        .expect("read root")
        .map(|entry| entry.expect("read entry").file_name())
        .collect::<Vec<_>>();
    assert_eq!(entries, vec![std::ffi::OsString::from("target")]);
}

#[test]
fn secure_open_flags_preserve_each_required_safety_property() {
    let directory = directory_open_flags();
    assert!(directory.contains(OFlags::DIRECTORY));
    assert!(directory.contains(OFlags::CLOEXEC));

    let read = read_open_flags();
    assert!(read.contains(OFlags::CLOEXEC));
    assert!(!read.contains(OFlags::WRONLY));

    let temp = temp_file_open_flags();
    assert!(temp.contains(OFlags::WRONLY));
    assert!(temp.contains(OFlags::CLOEXEC));
    assert!(temp.contains(OFlags::CREATE));
    assert!(temp.contains(OFlags::EXCL));
}

#[test]
fn secure_resolve_flags_forbid_links_and_root_escape() {
    let nested = secure_resolve_flags();
    assert!(nested.contains(ResolveFlags::BENEATH));
    assert!(nested.contains(ResolveFlags::NO_SYMLINKS));
    assert!(nested.contains(ResolveFlags::NO_MAGICLINKS));

    let anchor = secure_anchor_resolve_flags();
    assert!(!anchor.contains(ResolveFlags::BENEATH));
    assert!(anchor.contains(ResolveFlags::NO_SYMLINKS));
    assert!(anchor.contains(ResolveFlags::NO_MAGICLINKS));
}

#[test]
fn secure_error_classification_retries_only_expected_filesystem_states() {
    use std::io::ErrorKind;

    assert!(backup_name_is_taken(ErrorKind::AlreadyExists));
    assert!(!backup_name_is_taken(ErrorKind::PermissionDenied));
    assert!(child_directory_is_missing(ErrorKind::NotFound));
    assert!(!child_directory_is_missing(ErrorKind::NotADirectory));
    assert!(empty_dir_cleanup_is_complete(ErrorKind::NotFound));
    assert!(empty_dir_cleanup_is_complete(ErrorKind::DirectoryNotEmpty));
    assert!(!empty_dir_cleanup_is_complete(ErrorKind::PermissionDenied));
}

#[test]
fn secure_directory_walk_rejects_parent_and_symlink_segments() {
    let root = TempDirGuard::new("walk-containment");
    let outside = TempDirGuard::new("walk-outside");
    symlink(&outside.path, root.path.join("escape")).expect("create escape symlink");

    assert!(open_secure_dir_all(&root.path.join("..")).is_err());
    assert!(open_secure_dir_all(&root.path.join("escape").join("nested")).is_err());
    assert!(!outside.path.join("nested").exists());
}

#[test]
fn secure_read_returns_contents_and_mode_but_rejects_non_files() {
    let root = TempDirGuard::new("read");
    root.write("nested/value.txt", "payload");
    let root_fd = open_secure_dir_all(&root.path).expect("open secure root");

    let (contents, mode) = read_relative_file_secure(&root_fd, Path::new("nested/value.txt"))
        .expect("read regular file");
    assert_eq!(contents, b"payload");
    assert_ne!(mode & 0o600, 0, "fixture should retain owner access");
    assert!(read_relative_file_secure(&root_fd, Path::new("nested")).is_err());

    symlink("value.txt", root.path.join("nested/link.txt")).expect("create file symlink");
    assert!(read_relative_file_secure(&root_fd, Path::new("nested/link.txt")).is_err());
}

#[test]
fn secure_bounded_read_rejects_oversized_file_before_returning_bytes() {
    let root = TempDirGuard::new("bounded-read");
    root.write("value.txt", "12345");
    let root_fd = open_secure_dir_all(&root.path).expect("open secure root");

    let error = read_relative_file_secure_bounded(&root_fd, Path::new("value.txt"), 4)
        .expect_err("oversized file should fail");

    assert!(error.to_string().contains("4 byte limit"));
}

#[test]
fn secure_backup_directory_creation_uses_stable_unique_names() {
    let root = TempDirGuard::new("backups");
    let root_fd = open_secure_dir_all(&root.path).expect("open secure root");
    let (first, first_fd) = create_backup_dir_secure(&root_fd).expect("create first backup");
    let (second, second_fd) = create_backup_dir_secure(&root_fd).expect("create second backup");

    assert!(first.to_string_lossy().starts_with("Backup-"));
    assert_eq!(second, PathBuf::from(format!("{}-001", first.display())));
    assert!(root.path.join(&first).is_dir());
    assert!(root.path.join(&second).is_dir());
    assert!(rustix::io::fcntl_getfd(&first_fd)
        .expect("first backup fd flags")
        .contains(rustix::io::FdFlags::CLOEXEC));
    assert!(rustix::io::fcntl_getfd(&second_fd)
        .expect("second backup fd flags")
        .contains(rustix::io::FdFlags::CLOEXEC));
}

#[test]
fn secure_backup_creation_propagates_non_collision_errors() {
    let root = TempDirGuard::new("backup-error");
    let plain_file = root.path.join("not-a-directory");
    fs::write(&plain_file, "payload").expect("write plain file");
    let invalid_root = fs::File::open(&plain_file).expect("open plain file").into();

    let error = create_backup_dir_secure(&invalid_root)
        .expect_err("a regular file descriptor cannot contain backup directories");

    assert!(error.to_string().contains("create secure backup directory"));
}

#[test]
fn secure_parent_creation_does_not_treat_wrong_file_types_as_missing() {
    let root = TempDirGuard::new("parent-type");
    root.write("blocked", "plain file");
    let root_fd = open_secure_dir_all(&root.path).expect("open secure root");

    let error = write_relative_file_atomic_secure(
        &root_fd,
        Path::new("blocked/value.txt"),
        b"payload",
        0o600,
    )
    .expect_err("a regular file cannot be opened as a parent directory");

    assert!(error.to_string().contains("open secure directory blocked"));
}

#[test]
fn secure_empty_directory_cleanup_propagates_unexpected_errors() {
    let root = TempDirGuard::new("cleanup-error");
    let plain_file = root.path.join("not-a-directory");
    fs::write(&plain_file, "payload").expect("write plain file");
    let invalid_root = fs::File::open(&plain_file).expect("open plain file").into();

    let error = remove_empty_relative_dirs_secure(&invalid_root, Path::new("nested/value.txt"))
        .expect_err("cleanup beneath a regular file descriptor should fail");

    assert!(error.to_string().contains("remove secure directory nested"));
}

#[test]
fn secure_removal_deletes_files_and_only_empty_parent_directories() {
    let root = TempDirGuard::new("removal");
    root.write("tree/empty/value.txt", "payload");
    root.write("tree/keep.txt", "keep");
    let root_fd = open_secure_dir_all(&root.path).expect("open secure root");

    remove_relative_file_secure(&root_fd, Path::new("tree/empty/value.txt"))
        .expect("remove nested file");
    remove_empty_relative_dirs_secure(&root_fd, Path::new("tree/empty/value.txt"))
        .expect("remove empty parents");

    assert!(!root.path.join("tree/empty").exists());
    assert!(root.path.join("tree/keep.txt").is_file());
    remove_relative_file_secure(&root_fd, Path::new("tree/keep.txt")).expect("remove kept file");
    remove_relative_dir_secure(&root_fd, Path::new("tree")).expect("remove final directory");
    assert!(!root.path.join("tree").exists());
}
