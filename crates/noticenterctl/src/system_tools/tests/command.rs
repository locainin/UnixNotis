use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::{SystemTime, UNIX_EPOCH};

use super::super::{command, command_from_spec, routing::use_fake_tool_bin};
use unixnotis_core::CommandSpec;

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

#[test]
fn production_lookup_returns_the_core_resolved_trusted_program() {
    let expected = unixnotis_core::util::trusted_system_program_path("sh")
        .expect("find sh in a trusted system directory");

    assert_eq!(
        super::super::lookup::trusted_program_path("sh"),
        Some(expected)
    );
    let path_like_name = format!("bin{}sh", std::path::MAIN_SEPARATOR);
    assert_eq!(
        super::super::lookup::trusted_program_path(&path_like_name),
        None
    );
}

#[test]
fn typed_command_preserves_literal_arguments_and_environment() {
    let root = TempDirGuard::new("typed");
    root.write_executable("printf", "#!/bin/sh\nexit 0\n");
    let _tools = use_fake_tool_bin(&root.path);
    let spec = CommandSpec::direct("printf", ["battery|charging"])
        .with_env("WIDGET_MODE", "literal value");

    let command = command_from_spec(&spec).expect("typed trusted command");

    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        vec![std::ffi::OsStr::new("battery|charging")]
    );
    assert_eq!(
        command
            .get_envs()
            .find(|(name, _)| *name == "WIDGET_MODE")
            .and_then(|(_, value)| value),
        Some(std::ffi::OsStr::new("literal value"))
    );
}

#[test]
fn typed_trusted_command_rejects_shell_mode() {
    let error = command_from_spec(&CommandSpec::shell("printf unsafe"))
        .expect_err("trusted tools must not invoke a shell");

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
}
