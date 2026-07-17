use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::{SystemTime, UNIX_EPOCH};

use super::super::{command, program_path, routing::use_fake_tool_bin, tokio_command};

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
            "unixnotis-daemon-tools-{label}-{}-{stamp}",
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
fn trusted_std_command_uses_explicit_test_fake_bin() {
    let root = TempDirGuard::new("std");
    root.write_executable("systemctl", "#!/bin/sh\nexit 0\n");
    let _tools = use_fake_tool_bin(&root.path);

    let status = command("systemctl")
        .expect("trusted systemctl")
        .status()
        .expect("run fake systemctl");

    assert!(status.success());
}

#[tokio::test]
async fn trusted_tokio_command_uses_explicit_test_fake_bin() {
    let root = TempDirGuard::new("tokio");
    root.write_executable("pgrep", "#!/bin/sh\nexit 0\n");
    let _tools = use_fake_tool_bin(&root.path);

    let status = tokio_command("pgrep")
        .expect("trusted pgrep")
        .status()
        .await
        .expect("run fake pgrep");

    assert!(status.success());
}

#[test]
fn trusted_program_path_rejects_path_like_names() {
    let root = TempDirGuard::new("separator");
    root.write_executable("kill", "#!/bin/sh\nexit 0\n");
    let _tools = use_fake_tool_bin(&root.path);

    let error = program_path("./kill").expect_err("path-like program should be rejected");

    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
}
