//! Filesystem validation helpers for preset import and export
//!
//! These checks keep path validation separate from the actual disk read and
//! write helpers so the guard logic stays easier to review

use anyhow::{anyhow, Context, Result};
#[cfg(target_os = "linux")]
use rustix::fs::fstat;
use std::fs;
#[cfg(target_os = "linux")]
use std::os::fd::OwnedFd;
#[cfg(target_os = "linux")]
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

use super::super::pathing::normalize_relative_path;

pub(crate) fn ensure_safe_target_path(config_dir: &Path, relative_path: &Path) -> Result<PathBuf> {
    // Normalize first so later checks only deal with one clean relative path form
    let relative_path = normalize_relative_path(relative_path)?;
    // Join stays local only if every live path segment under the root is a real directory
    let target_path = config_dir.join(&relative_path);

    // Existing symlink components could redirect writes outside the config tree
    let mut probe = config_dir.to_path_buf();
    for component in relative_path.components() {
        if let Component::Normal(part) = component {
            probe.push(part);
            if !probe.exists() {
                // A missing tail segment is fine because nothing can redirect through it yet
                break;
            }

            let metadata = fs::symlink_metadata(&probe)
                .with_context(|| format!("inspect target path {}", probe.display()))?;
            if metadata.file_type().is_symlink() {
                // Reject the whole import target once a single segment can jump outside the root
                return Err(anyhow!(
                    "preset import blocked because this path leaves the UnixNotis config directory through a symlink: {}",
                    probe.display()
                ));
            }
        }
    }

    Ok(target_path)
}

pub(crate) fn ensure_no_symlink_ancestors(path: &Path) -> Result<()> {
    // A symlink anywhere on the live config root path can redirect all later writes
    let mut probe = PathBuf::new();
    for component in path.components() {
        match component {
            // Keep any drive prefix intact on platforms that use one
            Component::Prefix(prefix) => probe.push(prefix.as_os_str()),
            // Rebuild the absolute path one segment at a time from the filesystem root
            Component::RootDir => probe.push(Path::new("/")),
            // `.` has no effect on the real target path
            Component::CurDir => {}
            Component::ParentDir => {
                // Parent segments here would make the root check itself ambiguous
                return Err(anyhow!(
                    "path contains unexpected parent traversal: {}",
                    path.display()
                ));
            }
            // Normal path parts are checked one by one so a linked parent cannot hide deeper hops
            Component::Normal(part) => probe.push(part),
        }

        if !probe.exists() {
            // Once a component does not exist yet, later symlink checks cannot inspect deeper
            break;
        }

        let metadata = fs::symlink_metadata(&probe)
            .with_context(|| format!("inspect path component {}", probe.display()))?;
        if metadata.file_type().is_symlink() {
            // Stop before import writes into a root that already points somewhere else
            return Err(anyhow!(
                "preset import blocked because the UnixNotis config directory path goes through a symlink: {}",
                probe.display()
            ));
        }
    }

    Ok(())
}

#[cfg(target_os = "linux")]
pub(crate) fn ensure_dir_fd_matches_live_path(root_dir: &OwnedFd, live_path: &Path) -> Result<()> {
    let live_metadata = fs::metadata(live_path)
        .with_context(|| format!("inspect live config directory {}", live_path.display()))?;
    let fd_stat = fstat(root_dir)
        .with_context(|| format!("stat open config directory {}", live_path.display()))?;

    // If the opened dir inode no longer matches the visible path, later writes would land elsewhere
    if live_metadata.dev() != fd_stat.st_dev || live_metadata.ino() != fd_stat.st_ino {
        return Err(anyhow!(
            "preset import blocked because the UnixNotis config directory changed during import: {}",
            live_path.display()
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "linux")]
    use super::ensure_dir_fd_matches_live_path;
    use super::{ensure_no_symlink_ancestors, ensure_safe_target_path};
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
                "unixnotis-preset-filesystem-checks-{}-{}-{}",
                name, stamp, serial
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
}
