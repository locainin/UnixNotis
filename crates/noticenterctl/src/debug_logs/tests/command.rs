use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use super::super::command::follow_debug_logs;
use super::super::journal::{
    follow_user_unit_logs, journal_has_user_unit_logs, journalctl_is_available,
};
use crate::system_tools::use_fake_tool_bin;

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

    fn remove(name: &'static str) -> Self {
        let previous = std::env::var_os(name);
        std::env::remove_var(name);
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
            "unixnotis-debug-logs-{label}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    fn write_journalctl(&self, script: &str) {
        let path = self.path.join("journalctl");
        fs::write(&path, script).expect("write journalctl stub");
        let mut permissions = fs::metadata(&path)
            .expect("journalctl metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("chmod journalctl stub");
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("debug log env lock")
}

#[test]
fn journalctl_availability_tracks_executable_status() {
    let _lock = env_lock();
    let root = TempDirGuard::new("availability");
    root.write_journalctl("#!/bin/sh\nexit 0\n");
    let _tools = use_fake_tool_bin(&root.path);

    assert!(journalctl_is_available());

    root.write_journalctl("#!/bin/sh\nexit 3\n");

    assert!(!journalctl_is_available());
}

#[test]
fn journal_probe_reports_user_unit_presence_from_exit_status() {
    let _lock = env_lock();
    let root = TempDirGuard::new("probe");
    root.write_journalctl("#!/bin/sh\nexit 0\n");
    let _tools = use_fake_tool_bin(&root.path);

    assert!(journal_has_user_unit_logs("unixnotis-daemon.service").expect("probe success"));

    root.write_journalctl("#!/bin/sh\nexit 7\n");

    assert!(!journal_has_user_unit_logs("unixnotis-daemon.service").expect("probe failure"));
}

#[test]
fn journal_follow_returns_error_for_nonzero_journalctl_exit() {
    let _lock = env_lock();
    let root = TempDirGuard::new("follow-failure");
    root.write_journalctl("#!/bin/sh\nexit 11\n");
    let _tools = use_fake_tool_bin(&root.path);

    let error = follow_user_unit_logs("unixnotis-daemon.service").expect_err("follow should fail");

    assert!(error.to_string().contains("journalctl exited with status"));
}

#[test]
fn follow_debug_logs_requires_journalctl_on_path() {
    let _lock = env_lock();
    let root = TempDirGuard::new("missing");
    let _tools = use_fake_tool_bin(&root.path);
    let _unit = EnvGuard::remove("UNIXNOTIS_DAEMON_UNIT");

    let error = follow_debug_logs().expect_err("missing journalctl should fail");

    assert!(error.to_string().contains("journalctl is not available"));
}

#[test]
fn follow_debug_logs_rejects_missing_user_unit_logs_before_following() {
    let _lock = env_lock();
    let root = TempDirGuard::new("no-logs");
    root.write_journalctl("#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then exit 0; fi\nexit 4\n");
    let _tools = use_fake_tool_bin(&root.path);
    let _unit = EnvGuard::set("UNIXNOTIS_DAEMON_UNIT", "custom.service");

    let error = follow_debug_logs().expect_err("missing unit logs should fail");

    assert!(error.to_string().contains("custom.service"));
    assert!(error.to_string().contains("debug panel open will continue"));
}

#[test]
fn follow_debug_logs_runs_probe_then_follow_for_available_unit() {
    let _lock = env_lock();
    let root = TempDirGuard::new("follow-success");
    let calls = root.path.join("calls");
    root.write_journalctl(&format!(
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> {calls:?}\nexit 0\n"
    ));
    let _tools = use_fake_tool_bin(&root.path);
    let _unit = EnvGuard::set("UNIXNOTIS_DAEMON_UNIT", "custom.service");

    follow_debug_logs().expect("follow should succeed");

    let calls = fs::read_to_string(calls).expect("read journalctl calls");
    assert!(calls.contains("--version"));
    assert!(calls.contains("--user --no-pager -n 1 -u custom.service -o cat"));
    assert!(calls.contains("--user -f -u custom.service -o cat"));
}

#[test]
fn journalctl_lookup_ignores_inherited_path_entries() {
    let _lock = env_lock();
    let root = TempDirGuard::new("path-hijack");
    let marker = root.path.join("marker");
    root.write_journalctl(&format!("#!/bin/sh\nprintf hit > {marker:?}\nexit 0\n"));
    let _path = EnvGuard::set("PATH", &root.path);
    let empty_tools = TempDirGuard::new("trusted-empty");
    let _tools = use_fake_tool_bin(&empty_tools.path);

    assert!(!journalctl_is_available());
    assert!(!marker.exists());
}
