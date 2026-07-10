use super::{command, program_path, use_fake_tool_bin};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn trusted_command_ignores_inherited_path_entries() {
    let _lock = crate::tests::env::test_env_lock();
    let root = test_root("trusted-tool-path-ignore");
    let fake_bin = root.join("bin");
    fs::create_dir_all(&fake_bin).expect("fake bin");
    write_executable(&fake_bin.join("unixnotis-fake-tool"), "#!/bin/sh\nexit 0\n");
    let _path = EnvGuard::set("PATH", &fake_bin);

    let err = command("unixnotis-fake-tool").expect_err("PATH fake must not be trusted");

    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn trusted_command_uses_explicit_test_fake_bin() {
    let root = test_root("trusted-tool-fake-bin");
    let fake_bin = root.join("bin");
    fs::create_dir_all(&fake_bin).expect("fake bin");
    write_executable(&fake_bin.join("unixnotis-fake-tool"), "#!/bin/sh\nexit 0\n");
    let _fake = use_fake_tool_bin(&fake_bin);

    let status = command("unixnotis-fake-tool")
        .expect("fake tool command")
        .status()
        .expect("fake tool status");

    assert!(status.success());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn trusted_program_rejects_empty_names() {
    let root = test_root("trusted-tool-empty-name");
    fs::create_dir_all(&root).expect("fake bin");
    let _fake = use_fake_tool_bin(&root);

    let err = program_path("").expect_err("empty tool name must be rejected");

    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn trusted_program_rejects_names_with_path_separators() {
    let root = test_root("trusted-tool-path-name");
    fs::create_dir_all(&root).expect("fake bin");
    write_executable(&root.join("tool"), "#!/bin/sh\nexit 0\n");
    let _fake = use_fake_tool_bin(&root);

    let err = program_path("./tool").expect_err("path-like tool name must be rejected");

    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn trusted_program_rejects_regular_files_without_execute_bits() {
    let root = test_root("trusted-tool-not-executable");
    fs::create_dir_all(&root).expect("fake bin");
    fs::write(root.join("tool"), "plain data").expect("fake tool");
    let _fake = use_fake_tool_bin(&root);

    let err = program_path("tool").expect_err("non-executable tool must be rejected");

    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn trusted_program_accepts_files_with_only_execute_bits() {
    let root = test_root("trusted-tool-execute-only");
    fs::create_dir_all(&root).expect("fake bin");
    let tool = root.join("tool");
    fs::write(&tool, "plain data").expect("fake tool");
    fs::set_permissions(&tool, fs::Permissions::from_mode(0o111)).expect("fake tool mode");
    let _fake = use_fake_tool_bin(&root);

    let resolved = program_path("tool").expect("execute-only tool should resolve");

    assert_eq!(resolved, tool);
    let _ = fs::remove_dir_all(root);
}

fn write_executable(path: &std::path::Path, contents: &str) {
    fs::write(path, contents).expect("fake tool");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("fake tool mode");
}

fn test_root(name: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "unixnotis-installer-{name}-{}-{stamp}",
        std::process::id()
    ))
}

struct EnvGuard {
    name: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn set(name: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
        let previous = std::env::var_os(name);
        std::env::set_var(name, value);
        Self { name, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => std::env::set_var(self.name, value),
            None => std::env::remove_var(self.name),
        }
    }
}
