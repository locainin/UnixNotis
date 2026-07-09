use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};

use crate::actions::ActionContext;
use crate::detect::Detection;
use crate::events::UiMessage;
use crate::model::ActionMode;
use crate::paths::InstallPaths;
use crate::service_manager::ServiceManager;

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
    let _lock = crate::tests::env::test_env_lock();
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
fn pid_alive_reports_zero_pid_as_not_alive() {
    let alive = pid_alive(0).expect("zero pid probe should still run");

    // PID 0 targets the caller's process group, not one daemon process
    assert!(!alive);
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
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
        .expect("ps should inspect current process");
    let expected = String::from_utf8_lossy(&output.stdout).trim().to_string();

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

fn write_executable(path: &std::path::Path, contents: &str) {
    use std::os::unix::fs::PermissionsExt;

    std::fs::write(path, contents).expect("fake command");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .expect("fake command mode");
}
