use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEST_TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub(in crate::preset::export) struct TempDirGuard {
    pub(in crate::preset::export) path: PathBuf,
}

impl TempDirGuard {
    pub(in crate::preset::export) fn new(name: &str) -> Self {
        // Unique temp roots keep preset tests isolated from each other
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock moved backwards")
            .as_nanos();
        let serial = TEST_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("unixnotis-preset-export-{name}-{stamp}-{serial}"));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    pub(in crate::preset::export) fn write(&self, relative_path: &str, contents: &str) {
        // Helper keeps test setup focused on intent instead of path plumbing
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
