use std::fs;
use std::os::unix::fs::symlink;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};

use crate::detect::Detection;
use crate::events::{UiMessage, WorkerEvent};
use crate::model::ActionMode;
use crate::paths::InstallPaths;
use crate::service_manager::{use_fake_command_bin, ServiceManager, MANAGED_DIRECTORY_MARKER};

use super::{check_install_state, check_install_state_step, ActionContext};

#[test]
fn dinit_artifact_backed_enablement_does_not_log_missing_enabled_command_error() {
    let _lock = crate::tests::env::test_env_lock();
    let root = test_root("dinit-artifact-enabled-state");
    let service_root = root.join("dinit.d");
    let boot_dir = service_root.join("boot.d");
    fs::create_dir_all(&boot_dir).expect("boot dir");
    fs::write(
        service_root.join("unixnotis-daemon"),
        "type = process\ncommand = /tmp/bin/unixnotis-daemon\n",
    )
    .expect("service file");
    symlink("../unixnotis-daemon", boot_dir.join("unixnotis-daemon")).expect("boot symlink");

    let paths = InstallPaths {
        repo_root: repo_root(),
        bin_dir: root.join("bin"),
        service: ServiceManager::dinit_user(service_root),
    };

    let state = check_install_state(&paths);

    assert!(state.service_enabled);
    assert!(state.service_enabled_error.is_none());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn install_state_rejects_foreign_runit_service_directory() {
    let _lock = crate::tests::env::test_env_lock();
    let root = test_root("runit-foreign-install-state");
    let service_root = root.join("service");
    let service_dir = service_root.join("unixnotis-daemon");
    fs::create_dir_all(&service_dir).expect("foreign service dir");
    fs::write(service_dir.join("run"), "#!/bin/sh\n").expect("foreign run script");
    fs::create_dir_all(root.join("bin")).expect("bin dir");

    let paths = InstallPaths {
        repo_root: repo_root(),
        bin_dir: root.join("bin"),
        service: ServiceManager::runit_user(service_root),
    };

    let state = check_install_state(&paths);

    assert!(!state.service_artifact_exists);
    assert!(!service_dir.join(MANAGED_DIRECTORY_MARKER).exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn selected_backend_artifacts_do_not_count_as_cross_backend_conflict() {
    let _lock = crate::tests::env::test_env_lock();
    let root = test_root("selected-backend-conflict");
    // Discovery reads global HOME/XDG vars, so every path stays inside the test root
    let _env = service_scan_env(&root);
    // Host managers must not affect whether this filesystem-only regression passes
    let _fake_commands = fake_inactive_manager_commands(&root);
    let service_root = root.join("home").join(".config").join("dinit.d");
    write_dinit_artifacts(&service_root);

    let paths = InstallPaths {
        repo_root: repo_root(),
        bin_dir: root.join("bin"),
        service: ServiceManager::dinit_user(service_root),
    };

    let state = check_install_state(&paths);

    // Reinstalling the selected backend is valid and should not look like manager drift
    assert!(state.service_artifact_exists);
    assert!(state.service_conflicts.is_empty());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn different_backend_artifacts_are_reported_as_install_conflict() {
    let _lock = crate::tests::env::test_env_lock();
    let root = test_root("different-backend-artifact-conflict");
    // Match real discovery paths so the conflict scanner sees the dinit artifacts
    let _env = service_scan_env(&root);
    // Keep the test focused on artifact conflicts, not active runtime probes
    let _fake_commands = fake_inactive_manager_commands(&root);
    let dinit_root = root.join("home").join(".config").join("dinit.d");
    write_dinit_artifacts(&dinit_root);

    let paths = InstallPaths {
        repo_root: repo_root(),
        bin_dir: root.join("bin"),
        service: ServiceManager::systemd_user(
            root.join("home")
                .join(".config")
                .join("systemd")
                .join("user"),
        ),
    };

    let state = check_install_state(&paths);

    assert_eq!(state.service_conflicts.len(), 1);
    assert_eq!(state.service_conflicts[0].manager_label, "dinit --user");
    assert!(state.service_conflicts[0].installed);
    assert!(!state.service_conflicts[0].active);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn install_check_blocks_when_different_backend_is_active() {
    let _lock = crate::tests::env::test_env_lock();
    let root = test_root("different-backend-active-conflict");
    let _env = service_scan_env(&root);
    let fake_bin = root.join("fake-bin");
    let fake_systemctl = fake_bin.join("systemctl");
    fs::create_dir_all(&fake_bin).expect("fake bin");
    for command in ["dinitctl", "sv", "s6-svstat"] {
        // Only systemd should look active; every other backend probe should stay inactive
        let path = fake_bin.join(command);
        fs::write(&path, "#!/bin/sh\nexit 1\n").expect("fake inactive command");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
            .expect("chmod fake inactive command");
    }
    fs::write(
        &fake_systemctl,
        "#!/bin/sh\ncase \" $* \" in *\" is-active \"*) exit 0 ;; *) exit 1 ;; esac\n",
    )
    .expect("fake systemctl");
    fs::set_permissions(&fake_systemctl, fs::Permissions::from_mode(0o755))
        .expect("chmod fake systemctl");
    let _fake_bin = use_fake_command_bin(&fake_bin);
    let paths = InstallPaths {
        repo_root: repo_root(),
        bin_dir: root.join("bin"),
        service: ServiceManager::dinit_user(root.join("home").join(".config").join("dinit.d")),
    };
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let (log_tx, log_rx) = mpsc::sync_channel::<UiMessage>(16);
    let mut ctx = ActionContext {
        detection: &detection,
        paths: &paths,
        install_state: None,
        log_tx,
        action_mode: ActionMode::Install,
        restore_backup: None,
        service_reload_required: Arc::new(AtomicBool::new(false)),
    };

    let err = check_install_state_step(&mut ctx).expect_err("active conflict should block install");

    // Active conflicts are blocked even when the other backend has no safe artifact on disk
    assert!(err
        .to_string()
        .contains("already appears managed by another service manager"));
    let logs = log_rx.try_iter().collect::<Vec<_>>();
    assert!(logs.iter().any(|event| matches!(
        event,
        UiMessage::Worker(WorkerEvent::LogLine(line))
            if line.contains("UnixNotis is active under systemd --user")
    )));

    let _ = fs::remove_dir_all(root);
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .to_path_buf()
}

fn test_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("unixnotis-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    root
}

fn write_dinit_artifacts(service_root: &Path) {
    let boot_dir = service_root.join("boot.d");
    fs::create_dir_all(&boot_dir).expect("boot dir");
    // The file body only needs to satisfy the artifact shape check, not start a real daemon
    fs::write(
        service_root.join("unixnotis-daemon"),
        "type = process\ncommand = /tmp/bin/unixnotis-daemon\n",
    )
    .expect("service file");
    symlink("../unixnotis-daemon", boot_dir.join("unixnotis-daemon")).expect("boot symlink");
}

fn service_scan_env(root: &Path) -> Vec<EnvGuard> {
    // Clear backend override env vars so alternate manager discovery uses normal user roots
    vec![
        EnvGuard::set("HOME", root.join("home").display().to_string()),
        EnvGuard::set("USER", "unixnotis-test"),
        EnvGuard::set(
            "XDG_CONFIG_HOME",
            root.join("home").join(".config").display().to_string(),
        ),
        EnvGuard::remove("UNIXNOTIS_RUNIT_SERVICE_DIR"),
        EnvGuard::remove("UNIXNOTIS_S6_DATA_DIR"),
        EnvGuard::remove("UNIXNOTIS_S6RC_LIVE_DIR"),
        EnvGuard::remove("SVDIR"),
    ]
}

fn fake_inactive_manager_commands(root: &Path) -> impl Drop {
    let fake_bin = root.join("fake-inactive-bin");
    fs::create_dir_all(&fake_bin).expect("fake inactive bin");
    for command in ["systemctl", "dinitctl", "sv", "s6-svstat"] {
        // Exit 1 models a healthy inactive service for every active-state probe style
        let path = fake_bin.join(command);
        fs::write(&path, "#!/bin/sh\nexit 1\n").expect("fake inactive command");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
            .expect("chmod fake inactive command");
    }
    // Active probes are command-backed, so route them away from the host managers
    use_fake_command_bin(&fake_bin)
}

struct EnvGuard {
    key: &'static str,
    old: Option<String>,
}

impl EnvGuard {
    fn set(key: &'static str, value: impl Into<String>) -> Self {
        // Cross-backend scans use global env, so callers hold the shared env test lock
        let old = std::env::var(key).ok();
        std::env::set_var(key, value.into());
        Self { key, old }
    }

    fn remove(key: &'static str) -> Self {
        let old = std::env::var(key).ok();
        std::env::remove_var(key);
        Self { key, old }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // Restore process-wide env so later tests do not inherit fake backend roots
        if let Some(old) = &self.old {
            std::env::set_var(self.key, old);
        } else {
            std::env::remove_var(self.key);
        }
    }
}
