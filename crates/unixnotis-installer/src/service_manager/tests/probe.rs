use std::fs;
use std::os::unix::fs::symlink;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::service_manager::{use_fake_command_bin, CommandSpec, ServiceProbe};

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
            "unixnotis-service-probe-{label}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    fn link_shell(&self, name: &str) {
        // Stable shell links avoid transient executable-busy failures during mutation runs
        symlink("/bin/sh", self.path.join(name)).expect("link fake probe tool");
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn stdout_probe_uses_parser_only_after_successful_command() {
    let root = TempDirGuard::new("success");
    root.link_shell("probe-tool");
    let _tools = use_fake_command_bin(&root.path);
    let probe = ServiceProbe::stdout(
        CommandSpec::new("probe", "probe-tool", ["-c", "printf 'true\\n'; exit 0"]),
        |stdout| stdout.trim() == "true",
    );

    let active = probe.evaluate().expect("probe should run");

    // Successful stdout probes are allowed to derive active state from command output
    assert!(active);
}

#[test]
fn stdout_probe_treats_failed_command_as_inactive_even_with_matching_output() {
    let root = TempDirGuard::new("failure");
    root.link_shell("probe-tool");
    let _tools = use_fake_command_bin(&root.path);
    let probe = ServiceProbe::stdout(
        CommandSpec::new("probe", "probe-tool", ["-c", "printf 'true\\n'; exit 1"]),
        |stdout| stdout.trim() == "true",
    );

    let active = probe.evaluate().expect("probe should run");

    // Command failure means the manager did not provide trustworthy status output
    assert!(!active);
}
