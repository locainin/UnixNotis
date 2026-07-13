#[cfg(target_os = "linux")]
use super::super::ensure_dir_fd_matches_live_path;
use super::super::{ensure_no_symlink_ancestors, ensure_safe_target_path};
#[cfg(target_os = "linux")]
use crate::preset::filesystem::open_secure_dir_all;
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
            "unixnotis-preset-filesystem-checks-{name}-{stamp}-{serial}"
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(unix)]
#[test]
fn ensure_no_symlink_ancestors_rejects_symlinked_parent_path() {
    // A symlinked ancestor can redirect the whole config root outside the expected tree
    let root = TempDirGuard::new("symlink-ancestor");
    let real_xdg = root.path.join("real-xdg");
    let linked_xdg = root.path.join("linked-xdg");
    fs::create_dir_all(real_xdg.join("unixnotis")).expect("create real config dir");
    std::os::unix::fs::symlink(&real_xdg, &linked_xdg).expect("create xdg symlink");

    let error =
        ensure_no_symlink_ancestors(&linked_xdg.join("unixnotis")).expect_err("reject symlink");
    assert!(error
        .to_string()
        .contains("config directory path goes through a symlink"));
}

#[cfg(unix)]
#[test]
fn ensure_safe_target_path_rejects_symlinked_child_path() {
    // A symlink inside the config tree must not be accepted as a real write target
    let root = TempDirGuard::new("symlink-child");
    let config_dir = root.path.join("unixnotis");
    let outside_dir = root.path.join("outside");
    fs::create_dir_all(&config_dir).expect("create config dir");
    fs::create_dir_all(&outside_dir).expect("create outside dir");
    std::os::unix::fs::symlink(&outside_dir, config_dir.join("assets"))
        .expect("create assets symlink");

    let error = ensure_safe_target_path(&config_dir, Path::new("assets/bg.png"))
        .expect_err("reject symlinked child");
    assert!(error
        .to_string()
        .contains("leaves the UnixNotis config directory through a symlink"));
}

#[cfg(target_os = "linux")]
#[test]
fn ensure_dir_fd_matches_live_path_rejects_root_move() {
    // A moved config root should not keep accepting writes through an old directory fd
    let root = TempDirGuard::new("root-move");
    let xdg = root.path.join("xdg");
    let config_dir = xdg.join("unixnotis");
    let moved_dir = root.path.join("moved-unixnotis");
    fs::create_dir_all(&config_dir).expect("create config dir");

    let config_root_fd = open_secure_dir_all(&config_dir).expect("open secure root");
    fs::rename(&config_dir, &moved_dir).expect("move config dir");
    fs::create_dir_all(&config_dir).expect("recreate config dir path");

    let error = ensure_dir_fd_matches_live_path(&config_root_fd, &config_dir)
        .expect_err("reject moved config root");
    assert!(error.to_string().contains("changed during import"));
}
