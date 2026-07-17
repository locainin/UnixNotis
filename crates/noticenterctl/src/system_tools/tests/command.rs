use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::{SystemTime, UNIX_EPOCH};

use super::super::{command, routing::use_fake_tool_bin};

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
            "unixnotis-noticenterctl-tools-{label}-{}-{stamp}",
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
fn trusted_command_uses_explicit_test_fake_bin() {
    let root = TempDirGuard::new("fake-bin");
    root.write_executable("journalctl", "#!/bin/sh\nexit 0\n");
    let _tools = use_fake_tool_bin(&root.path);

    let status = command("journalctl")
        .expect("trusted journalctl")
        .status()
        .expect("run fake journalctl");

    assert!(status.success());
}

#[test]
fn trusted_command_rejects_program_names_with_path_separators() {
    let root = TempDirGuard::new("separator");
    root.write_executable("tool", "#!/bin/sh\nexit 0\n");
    let _tools = use_fake_tool_bin(&root.path);

    let error = command("./tool").expect_err("path-like program should be rejected");

    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
}
