use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};

use crate::actions::ActionContext;
use crate::app::events::UiMessage;
use crate::detect::{DetectedDaemon, Detection, OwnerInfo};
use crate::model::ActionMode;
use crate::paths::InstallPaths;
use crate::service_manager::ServiceManager;
use crate::test_support::fs::write_executable;

use super::{
    is_systemd_unit_inactive, pid_alive, pid_matches_comm, stop_active_daemon,
    systemd_stop_error_is_satisfied_by_state, wait_for_exit,
};

#[test]
fn stop_active_daemon_errors_for_unmanaged_owner() {
    let detection = Detection {
        owner: Some(crate::detect::OwnerInfo {
            pid: None,
            comm: Some("unknown-daemon".to_string()),
        }),
        daemons: Vec::new(),
    };
    let paths = InstallPaths {
        repo_root: std::env::temp_dir(),
        bin_dir: std::env::temp_dir(),
        service: ServiceManager::systemd_user(std::env::temp_dir()),
    };
    let (tx, _rx) = mpsc::sync_channel::<UiMessage>(4);
    let mut ctx = ActionContext {
        detection: &detection,
        paths: &paths,
        install_state: None,
        log_tx: tx,
        action_mode: ActionMode::Install,
        restore_backup: None,
        service_reload_required: Arc::new(AtomicBool::new(false)),
    };

    let error = stop_active_daemon(&mut ctx).expect_err("unmanaged owner must block install");

    assert!(error.to_string().contains("not managed by a known unit"));
}

#[test]
fn stop_active_daemon_uses_owner_command_match_before_pid_fallback() {
    let root = fake_daemon_tool_root("owner-command-match");
    let state = root.join("kill-state");
    write_executable(
        &root.join("kill"),
        &format!(
            "#!/bin/sh\nif [ \"$1\" = \"-0\" ]; then if [ -e {0} ]; then exit 1; fi; : > {0}; fi\nexit 0\n",
            state.display()
        ),
    );
    write_executable(&root.join("ps"), "#!/bin/sh\nprintf 'mako\\n'\n");
    let _tools = crate::system_tools::use_fake_tool_bin(&root);
    let detection = known_daemon_detection("mako", false, Vec::new());
    let paths = test_install_paths();
    let (tx, _rx) = mpsc::sync_channel::<UiMessage>(8);
    let mut ctx = action_context(&detection, &paths, tx);

    stop_active_daemon(&mut ctx).expect("matching owner command should stop daemon");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn stop_active_daemon_skips_process_inspection_when_pid_is_already_gone() {
    let root = fake_daemon_tool_root("already-gone");
    let ps_marker = root.join("ps-ran");
    write_executable(&root.join("kill"), "#!/bin/sh\nexit 1\n");
    write_executable(
        &root.join("ps"),
        &format!("#!/bin/sh\nprintf hit > {}\nexit 0\n", ps_marker.display()),
    );
    let _tools = crate::system_tools::use_fake_tool_bin(&root);
    let detection = known_daemon_detection("mako", false, Vec::new());
    let paths = test_install_paths();
    let (tx, _rx) = mpsc::sync_channel::<UiMessage>(8);
    let mut ctx = action_context(&detection, &paths, tx);

    stop_active_daemon(&mut ctx).expect("already stopped process should be accepted");

    assert!(!ps_marker.exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn stop_active_daemon_accepts_natural_exit_after_command_mismatch() {
    let root = fake_daemon_tool_root("natural-exit");
    let state = root.join("kill-state");
    let ps_marker = root.join("ps-ran");
    write_executable(
        &root.join("kill"),
        &format!(
            "#!/bin/sh\nif [ -e {0} ]; then exit 1; fi\n: > {0}\nexit 0\n",
            state.display()
        ),
    );
    write_executable(
        &root.join("ps"),
        &format!(
            "#!/bin/sh\nprintf hit > {}\nprintf 'different-daemon\\n'\n",
            ps_marker.display()
        ),
    );
    let _tools = crate::system_tools::use_fake_tool_bin(&root);
    let detection = known_daemon_detection("mako", false, Vec::new());
    let paths = test_install_paths();
    let (tx, _rx) = mpsc::sync_channel::<UiMessage>(8);
    let mut ctx = action_context(&detection, &paths, tx);

    stop_active_daemon(&mut ctx).expect("natural process exit should satisfy stop");

    assert!(ps_marker.exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn stop_active_daemon_stops_unixnotis_without_disabling_its_unit() {
    let root = fake_daemon_tool_root("unixnotis-reinstall-stop");
    let calls = root.join("systemctl-calls");
    write_executable(
        &root.join("systemctl"),
        &format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> {}\n", calls.display()),
    );
    let _commands = crate::service_manager::use_fake_command_bin(&root);
    let _tools = crate::system_tools::use_fake_tool_bin(&root);
    let detection = known_daemon_detection("unixnotis-daemon", true, Vec::new());
    let paths = test_install_paths();
    let (tx, _rx) = mpsc::sync_channel::<UiMessage>(8);
    let mut ctx = action_context(&detection, &paths, tx);

    stop_active_daemon(&mut ctx).expect("reinstall stop should succeed");

    let calls = std::fs::read_to_string(&calls).expect("systemctl calls");
    assert_eq!(calls.trim(), "--user stop unixnotis-daemon.service");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn systemd_stop_error_can_continue_when_unit_is_inactive() {
    // A failed stop is acceptable only when systemd reports a non-running state
    assert!(systemd_stop_error_is_satisfied_by_state("inactive"));
}

#[test]
fn systemd_stop_error_can_continue_when_unit_is_failed() {
    // Failed units no longer own the notification bus, so reinstall may continue
    assert!(systemd_stop_error_is_satisfied_by_state("failed"));
}

#[test]
fn systemd_stop_error_still_fails_when_unit_stays_active() {
    assert!(!systemd_stop_error_is_satisfied_by_state("active"));
}

#[test]
fn systemd_stop_error_still_fails_when_unit_is_transitioning() {
    assert!(!systemd_stop_error_is_satisfied_by_state("deactivating"));
}

#[test]
fn systemd_stop_error_still_fails_when_state_is_empty() {
    // Empty output means the manager did not provide enough proof that stopping succeeded
    assert!(!systemd_stop_error_is_satisfied_by_state(""));
}

#[test]
fn systemd_stop_error_trims_state_output_before_matching() {
    // systemctl prints a trailing newline in normal output
    assert!(systemd_stop_error_is_satisfied_by_state(" inactive\n"));
    assert!(systemd_stop_error_is_satisfied_by_state("\tunknown "));
}

#[test]
fn systemd_stop_error_rejects_unrecognized_non_running_words() {
    // Only explicit systemd states should satisfy a failed stop
    assert!(!systemd_stop_error_is_satisfied_by_state("dead"));
    assert!(!systemd_stop_error_is_satisfied_by_state("stopped"));
}

#[test]
fn is_systemd_unit_inactive_reads_trusted_systemctl_state() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = std::env::temp_dir().join(format!(
        "unixnotis-daemon-systemctl-state-{}",
        std::process::id()
    ));
    let fake_bin = root.join("bin");
    std::fs::create_dir_all(&fake_bin).expect("fake bin");
    write_executable(
        &fake_bin.join("systemctl"),
        "#!/bin/sh\ncase \"$3\" in inactive.service) echo inactive; exit 3 ;; active.service) echo active; exit 0 ;; *) exit 1 ;; esac\n",
    );
    let _fake_tools = crate::system_tools::use_fake_tool_bin(&fake_bin);

    assert!(is_systemd_unit_inactive("inactive.service").expect("inactive state"));
    assert!(!is_systemd_unit_inactive("active.service").expect("active state"));
    let error =
        is_systemd_unit_inactive("missing.service").expect_err("empty failed status is an error");
    assert!(error
        .to_string()
        .contains("failed to read systemd unit state"));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn pid_alive_reports_current_process_as_alive() {
    let pid = std::process::id();

    // The current test process should always satisfy a kill -0 probe
    assert!(pid_alive(pid).expect("current pid probe"));
}

#[test]
fn pid_alive_reports_impossible_pid_as_not_alive() {
    let alive = pid_alive(u32::MAX).expect("invalid pid probe should still run");

    // A non-existent PID must not be treated as safe to signal
    assert!(!alive);
}

#[test]
fn pid_alive_probes_largest_valid_process_id() {
    let root = fake_daemon_tool_root("max-pid");
    write_executable(&root.join("kill"), "#!/bin/sh\nexit 0\n");
    let _tools = crate::system_tools::use_fake_tool_bin(&root);

    assert!(pid_alive(i32::MAX as u32).expect("largest valid pid probe"));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn pid_alive_reports_zero_pid_as_not_alive() {
    let alive = pid_alive(0).expect("zero pid probe should still run");

    // PID 0 targets the caller's process group, not one daemon process
    assert!(!alive);
}

#[test]
fn pid_alive_ignores_kill_from_inherited_path() {
    let _lock = crate::test_support::env::test_env_lock();
    let root =
        std::env::temp_dir().join(format!("unixnotis-daemon-kill-path-{}", std::process::id()));
    let path_bin = root.join("path-bin");
    let trusted_bin = root.join("trusted-bin");
    let marker = root.join("path-kill-ran");
    std::fs::create_dir_all(&path_bin).expect("path bin");
    std::fs::create_dir_all(&trusted_bin).expect("trusted bin");
    write_executable(
        &path_bin.join("kill"),
        &format!("#!/bin/sh\nprintf hit > {}\nexit 0\n", marker.display()),
    );
    write_executable(&trusted_bin.join("kill"), "#!/bin/sh\nexit 0\n");
    let _path = EnvGuard::set("PATH", &path_bin);
    let _tools = crate::system_tools::use_fake_tool_bin(&trusted_bin);

    assert!(pid_alive(std::process::id()).expect("trusted pid probe"));
    assert!(!marker.exists());

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn pid_matches_comm_rejects_wrong_process_name() {
    let pid = std::process::id();

    let matches = pid_matches_comm(pid, "definitely-not-unixnotis").expect("comm probe");

    // PID reuse protection depends on rejecting mismatched command names
    assert!(!matches);
}

#[test]
fn pid_matches_comm_accepts_current_process_name_from_ps() {
    let pid = std::process::id();
    let expected = std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .expect("proc should expose the current process name")
        .trim()
        .to_string();

    let matches = pid_matches_comm(pid, &expected).expect("comm probe");

    // A matching process name is the only case where stop logic may signal the PID
    assert!(matches);
}

#[test]
fn wait_for_exit_aborts_immediately_when_pid_name_no_longer_matches() {
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let paths = InstallPaths {
        repo_root: std::env::temp_dir(),
        bin_dir: std::env::temp_dir(),
        service: ServiceManager::systemd_user(std::env::temp_dir()),
    };
    let (tx, _rx) = mpsc::sync_channel::<UiMessage>(4);
    let mut ctx = ActionContext {
        detection: &detection,
        paths: &paths,
        install_state: None,
        log_tx: tx,
        action_mode: ActionMode::Install,
        restore_backup: None,
        service_reload_required: Arc::new(AtomicBool::new(false)),
    };

    let err = wait_for_exit(
        &mut ctx,
        std::process::id(),
        "definitely-not-current-process",
    )
    .expect_err("mismatched comm should abort");

    // The wait loop must fail before sleeping when PID reuse is detected
    assert!(err
        .to_string()
        .contains("no longer matches expected daemon"));
}

fn fake_daemon_tool_root(label: &str) -> std::path::PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock moved backwards")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "unixnotis-daemon-{label}-{}-{stamp}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("fake daemon tool bin");
    root
}

fn known_daemon_detection(name: &str, systemd_active: bool, running_pids: Vec<u32>) -> Detection {
    Detection {
        owner: Some(OwnerInfo {
            pid: Some(42),
            comm: Some(name.to_string()),
        }),
        daemons: vec![DetectedDaemon {
            name: name.to_string(),
            unit: format!("{name}.service"),
            systemd_active,
            systemd_error: None,
            running_pids,
            is_owner: true,
        }],
    }
}

fn test_install_paths() -> InstallPaths {
    InstallPaths {
        repo_root: std::env::temp_dir(),
        bin_dir: std::env::temp_dir(),
        service: ServiceManager::systemd_user(std::env::temp_dir()),
    }
}

fn action_context<'a>(
    detection: &'a Detection,
    paths: &'a InstallPaths,
    log_tx: mpsc::SyncSender<UiMessage>,
) -> ActionContext<'a> {
    ActionContext {
        detection,
        paths,
        install_state: None,
        log_tx,
        action_mode: ActionMode::Install,
        restore_backup: None,
        service_reload_required: Arc::new(AtomicBool::new(false)),
    }
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
