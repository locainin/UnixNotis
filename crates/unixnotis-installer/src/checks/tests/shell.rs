use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use super::{path_check_item, path_check_item_from, path_entries_match};
use crate::checks::CheckState;
use crate::paths::InstallPaths;
use crate::service_manager::ServiceManager;

struct EnvGuard {
    key: &'static str,
    old: Option<String>,
}

impl EnvGuard {
    fn set(key: &'static str, value: impl AsRef<str>) -> Self {
        let old = env::var(key).ok();
        env::set_var(key, value.as_ref());
        Self { key, old }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.old {
            // PATH is process-wide, so restore it before the next test runs
            Some(value) => env::set_var(self.key, value),
            None => env::remove_var(self.key),
        }
    }
}

#[test]
fn path_entries_match_accepts_exact_paths() {
    // Exact string matches should return before any canonical filesystem work
    assert!(path_entries_match(
        Path::new("/tmp/unixnotis-bin"),
        Path::new("/tmp/unixnotis-bin")
    ));
}

#[test]
fn shell_path_warns_when_bin_is_on_path_but_command_was_uninstalled() {
    let item = path_check_item_from("$HOME/.local/bin", true, false);

    // PATH can be correct after uninstall, so command presence is checked separately
    assert_eq!(item.state, CheckState::Warn);
    assert!(item.detail.contains("noticenterctl is not installed there"));
}

#[test]
fn shell_path_warns_when_fresh_shell_has_no_path_and_no_command() {
    let item = path_check_item_from("$HOME/.local/bin", false, false);

    // Fresh installs should explain both missing PATH and missing command state
    assert_eq!(item.state, CheckState::Warn);
    assert!(item.detail.contains("missing $HOME/.local/bin"));
    assert!(item.detail.contains("noticenterctl is not installed there"));
}

#[test]
fn shell_path_is_ok_only_when_path_and_command_are_both_present() {
    let item = path_check_item_from("$HOME/.local/bin", true, true);

    // Direct command usage is ready only when shell lookup and managed install agree
    assert_eq!(item.state, CheckState::Ok);
    assert!(item.detail.contains("noticenterctl is installed there"));
}

#[test]
fn path_check_item_reads_real_path_and_managed_command_state() {
    let _lock = crate::tests::env::test_env_lock();
    let root = test_root("shell-path-real-state");
    let bin_dir = root.join("bin");
    fs::create_dir_all(&bin_dir).expect("bin dir");
    fs::write(bin_dir.join("noticenterctl"), "#!/bin/sh\n").expect("managed command");
    let paths = InstallPaths {
        repo_root: root.clone(),
        bin_dir: bin_dir.clone(),
        service: ServiceManager::systemd_user(root.join("systemd")),
    };
    let _path = EnvGuard::set("PATH", bin_dir.to_string_lossy());

    let item = path_check_item(&paths);

    // The real check must require both PATH membership and the installed command path
    assert_eq!(item.state, CheckState::Ok);
    assert!(item.detail.contains("noticenterctl is installed there"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn path_check_item_warns_when_command_exists_but_path_is_missing() {
    let _lock = crate::tests::env::test_env_lock();
    let root = test_root("shell-path-missing-path");
    let bin_dir = root.join("bin");
    fs::create_dir_all(&bin_dir).expect("bin dir");
    fs::write(bin_dir.join("noticenterctl"), "#!/bin/sh\n").expect("managed command");
    let paths = InstallPaths {
        repo_root: root.clone(),
        bin_dir,
        service: ServiceManager::systemd_user(root.join("systemd")),
    };
    let _path = EnvGuard::set("PATH", root.join("other-bin").to_string_lossy());

    let item = path_check_item(&paths);

    // Installed command alone is not enough because a fresh shell cannot find it by name
    assert_eq!(item.state, CheckState::Warn);
    assert!(item.detail.contains("missing"));
    assert!(item.detail.contains("installed there"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn path_entries_match_accepts_canonical_symlinked_paths() {
    let root = test_root("shell-path-canonical");
    let real = root.join("real-bin");
    let linked = root.join("linked-bin");
    fs::create_dir_all(&real).expect("real bin");
    std::os::unix::fs::symlink(&real, &linked).expect("linked bin");

    // Canonical matching avoids a false warning when PATH uses a symlinked bin directory
    assert!(path_entries_match(&linked, &real));

    let _ = fs::remove_dir_all(root);
}

fn test_root(name: &str) -> PathBuf {
    let root = env::temp_dir().join(format!("unixnotis-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    root
}
