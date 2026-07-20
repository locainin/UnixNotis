use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) struct TempToolDir {
    path: PathBuf,
}

impl TempToolDir {
    pub(super) fn new(label: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock moved backwards")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "unixnotis-session-environment-{label}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temporary tool directory");
        Self { path }
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn create_dir(&self, relative: impl AsRef<Path>) -> PathBuf {
        let path = self.path.join(relative);
        fs::create_dir_all(&path).expect("create temporary directory");
        path
    }

    pub(super) fn write_file(&self, relative: impl AsRef<Path>, contents: &str) -> PathBuf {
        let path = self.path.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create temporary file parent");
        }
        fs::write(&path, contents).expect("write temporary file");
        path
    }

    pub(super) fn write_executable(&self, name: &str, contents: &str) {
        let path = self.path.join(name);
        fs::write(&path, contents).expect("write temporary tool");
        let mut permissions = fs::metadata(&path)
            .expect("read temporary tool metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("make temporary tool executable");
    }
}

impl Drop for TempToolDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
