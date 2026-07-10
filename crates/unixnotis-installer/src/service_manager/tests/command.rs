use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{use_fake_command_bin, CommandSpec};
use crate::tests::fs::write_executable;

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
            "unixnotis-service-command-{label}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    fn write_executable(&self, name: &str, contents: &str) {
        let path = self.path.join(name);
        write_executable(&path, contents);
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct EnvGuard {
    previous: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn prepend_path(path: &std::path::Path) -> Self {
        let previous = std::env::var_os("PATH");
        let old_path = previous.clone().unwrap_or_default();
        let new_path = format!("{}:{}", path.display(), old_path.to_string_lossy());
        std::env::set_var("PATH", new_path);
        Self { previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => std::env::set_var("PATH", value),
            None => std::env::remove_var("PATH"),
        }
    }
}

#[test]
fn command_spec_uses_explicit_fake_bin_for_backend_tools() {
    let root = TempDirGuard::new("fake-bin");
    root.write_executable("dinitctl", "#!/bin/sh\nexit 0\n");
    let _tools = use_fake_command_bin(&root.path);
    let spec = CommandSpec::new("dinitctl --user list", "dinitctl", ["--user", "list"]);

    let status = spec
        .to_command()
        .expect("build trusted command")
        .status()
        .expect("run fake dinitctl");

    assert!(status.success());
}

#[test]
fn command_spec_ignores_inherited_path_backend_tools() {
    let _lock = crate::tests::env::test_env_lock();
    let root = TempDirGuard::new("path-hijack");
    let marker = root.path.join("marker");
    root.write_executable(
        "sv",
        &format!("#!/bin/sh\nprintf hit > {:?}\nexit 0\n", marker),
    );
    let _path = EnvGuard::prepend_path(&root.path);
    let empty_tools = TempDirGuard::new("empty-trusted");
    let _tools = use_fake_command_bin(&empty_tools.path);
    let spec = CommandSpec::new("sv -V", "sv", ["-V"]);

    let error = spec
        .to_command()
        .expect_err("PATH-only backend tool should be rejected");

    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    assert!(!marker.exists());
}

#[test]
fn command_spec_accessors_preserve_display_and_program_names() {
    let spec = CommandSpec::new("service status", "managerctl", ["status"]);

    assert_eq!(spec.label(), "service status");
    assert_eq!(spec.program(), "managerctl");
}
