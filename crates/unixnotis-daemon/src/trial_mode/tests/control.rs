use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::system_tools::routing::use_fake_tool_bin;

use super::super::{OwnerInfo, RestoreAction};
use super::{build_restart_command, stop_active_owner, stop_via_process, stop_via_systemd};
use crate::cli::{Args, RestoreStrategy};

struct TempDirGuard {
    path: std::path::PathBuf,
}

impl TempDirGuard {
    fn new(label: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock moved backwards")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "unixnotis-trial-control-{label}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    fn write_executable(&self, name: &str, contents: &str) {
        let path = self.path.join(name);
        fs::write(&path, contents).expect("write fake tool");
        let mut permissions = fs::metadata(&path)
            .expect("fake tool metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("chmod fake tool");
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn restart_command_preserves_captured_argv_without_trusted_lookup() {
    let owner = OwnerInfo {
        unique_name: ":1.test".to_string(),
        pid: Some(42),
        comm: Some("mako".to_string()),
        args: Some(vec![
            "/opt/custom/mako".to_string(),
            "--config".to_string(),
            "mako.conf".to_string(),
        ]),
    };

    let (program, args) = build_restart_command(&owner, "mako").expect("captured argv");

    assert_eq!(program, "/opt/custom/mako");
    assert_eq!(args, ["--config", "mako.conf"]);
}

#[test]
fn restart_command_resolves_missing_argv_fallback_from_trusted_tools() {
    let root = TempDirGuard::new("trusted-fallback");
    root.write_executable("mako", "#!/bin/sh\nexit 0\n");
    let _tools = use_fake_tool_bin(&root.path);
    let owner = OwnerInfo {
        unique_name: ":1.test".to_string(),
        pid: Some(42),
        comm: Some("mako".to_string()),
        args: None,
    };

    let (program, args) = build_restart_command(&owner, "mako").expect("trusted fallback");

    assert_eq!(program, root.path.join("mako").display().to_string());
    assert!(args.is_empty());
}

#[test]
fn restart_command_rejects_missing_argv_fallback_when_not_trusted() {
    let root = TempDirGuard::new("untrusted-fallback");
    root.write_executable("mako", "#!/bin/sh\nexit 0\n");
    let _path = EnvPathGuard::prepend(&root.path);
    let empty_trusted = TempDirGuard::new("empty-trusted");
    let _tools = use_fake_tool_bin(&empty_trusted.path);
    let owner = OwnerInfo {
        unique_name: ":1.test".to_string(),
        pid: Some(42),
        comm: Some("mako".to_string()),
        args: None,
    };

    let error = build_restart_command(&owner, "mako")
        .expect_err("untrusted PATH fallback should be rejected");

    assert_eq!(
        error
            .downcast_ref::<std::io::Error>()
            .map(std::io::Error::kind),
        Some(std::io::ErrorKind::NotFound)
    );
}

#[tokio::test]
async fn stop_via_systemd_errors_when_trusted_systemctl_fails() {
    let root = TempDirGuard::new("systemctl-fails");
    root.write_executable("systemctl", "#!/bin/sh\nexit 9\n");
    let _tools = use_fake_tool_bin(&root.path);

    let error = stop_via_systemd("mako.service")
        .await
        .expect_err("systemctl failure should stop trial setup");

    assert!(error.to_string().contains("systemctl stop failed"));
}

#[tokio::test]
async fn stop_via_process_errors_when_trusted_kill_fails() {
    let root = TempDirGuard::new("kill-fails");
    root.write_executable("kill", "#!/bin/sh\nexit 8\n");
    let _tools = use_fake_tool_bin(&root.path);

    let error = stop_via_process(42)
        .await
        .expect_err("kill failure should stop trial setup");

    assert!(error.to_string().contains("failed to stop process"));
}

#[tokio::test]
async fn process_restore_is_fully_constructed_before_owner_is_stopped() {
    let root = TempDirGuard::new("prepare-before-stop");
    let marker = root.path.join("kill-was-called");
    root.write_executable(
        "kill",
        &format!("#!/bin/sh\ntouch -- '{}'\n", marker.display()),
    );
    let _tools = use_fake_tool_bin(&root.path);
    let args = Args {
        config: None,
        trial: true,
        restore: RestoreStrategy::Process,
        yes: true,
        restore_wait_ms: 1,
        check: false,
        run_seconds: None,
    };
    let owner = OwnerInfo {
        unique_name: ":1.test".to_string(),
        pid: Some(42),
        comm: Some("mako".to_string()),
        args: None,
    };

    stop_active_owner(&args, &owner)
        .await
        .expect_err("missing trusted restart program");

    assert!(!marker.exists());
}

#[tokio::test]
async fn stop_active_owner_returns_systemd_restore_action_in_auto_mode() {
    let root = TempDirGuard::new("auto-systemd");
    root.write_executable("systemctl", "#!/bin/sh\nexit 0\n");
    let _tools = use_fake_tool_bin(&root.path);
    let args = Args {
        config: None,
        trial: true,
        restore: RestoreStrategy::Auto,
        yes: true,
        restore_wait_ms: 1,
        check: false,
        run_seconds: None,
    };
    let owner = OwnerInfo {
        unique_name: ":1.test".to_string(),
        pid: Some(42),
        comm: Some("mako".to_string()),
        args: Some(vec!["/usr/bin/mako".to_string()]),
    };

    let action = stop_active_owner(&args, &owner)
        .await
        .expect("stop active owner")
        .expect("restore action");

    match action {
        RestoreAction::Systemd { unit } => assert_eq!(unit, "mako.service"),
        RestoreAction::Command { .. } => panic!("expected systemd restore action"),
    }
}

struct EnvPathGuard {
    previous: Option<std::ffi::OsString>,
}

impl EnvPathGuard {
    fn prepend(path: &std::path::Path) -> Self {
        let previous = std::env::var_os("PATH");
        let old_path = previous.clone().unwrap_or_default();
        let new_path = format!("{}:{}", path.display(), old_path.to_string_lossy());
        std::env::set_var("PATH", new_path);
        Self { previous }
    }
}

impl Drop for EnvPathGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => std::env::set_var("PATH", value),
            None => std::env::remove_var("PATH"),
        }
    }
}
