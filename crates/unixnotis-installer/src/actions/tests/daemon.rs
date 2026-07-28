use std::process::{Command, Stdio};
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
    is_systemd_unit_inactive, stop_active_daemon, systemd_stop_error_is_satisfied_by_state,
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
fn stop_active_daemon_terminates_the_exact_non_systemd_owner() {
    let sleep = unixnotis_core::util::trusted_system_program_path("sleep")
        .expect("find sleep in a trusted system directory");
    let mut child = Command::new(sleep)
        .arg("30")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn sleep daemon");
    let detection = Detection {
        owner: Some(OwnerInfo {
            pid: Some(child.id()),
            comm: Some("sleep".to_string()),
        }),
        daemons: vec![DetectedDaemon {
            name: "sleep".to_string(),
            unit: "sleep.service".to_string(),
            systemd_active: false,
            systemd_error: None,
            running_pids: vec![child.id()],
            is_owner: true,
        }],
    };
    let paths = test_install_paths();
    let (tx, _rx) = mpsc::sync_channel::<UiMessage>(8);
    let mut ctx = action_context(&detection, &paths, tx);

    stop_active_daemon(&mut ctx).expect("stable process stop should succeed");

    let status = child.wait().expect("reap stopped sleep daemon");
    assert!(!status.success());
}

#[test]
fn stop_active_daemon_stops_unixnotis_without_disabling_its_unit() {
    let root = fake_daemon_tool_root("unixnotis-reinstall-stop");
    let calls = root.join("systemctl-calls");
    write_executable(
        &root.join("systemctl"),
        &format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> {}\n", calls.display()),
    );
    let _commands = crate::service_manager::contract::command_routing::use_fake_command_bin(&root);
    let _tools = crate::system_tools::routing::use_fake_tool_bin(&root);
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
    let _fake_tools = crate::system_tools::routing::use_fake_tool_bin(&fake_bin);

    assert!(is_systemd_unit_inactive("inactive.service").expect("inactive state"));
    assert!(!is_systemd_unit_inactive("active.service").expect("active state"));
    let error =
        is_systemd_unit_inactive("missing.service").expect_err("empty failed status is an error");
    assert!(error
        .to_string()
        .contains("failed to read systemd unit state"));

    let _ = std::fs::remove_dir_all(root);
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
