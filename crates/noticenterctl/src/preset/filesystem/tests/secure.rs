use super::super::{open_secure_dir_all, write_relative_file_atomic_secure};
use std::fs;
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
            "unixnotis-preset-filesystem-secure-{}-{}-{}",
            name, stamp, serial
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
}
