use std::process::{Command, Stdio};
use std::sync::mpsc;

use crate::app::events::UiMessage;
use crate::detect::{DetectedDaemon, Detection, OwnerInfo};
use crate::test_support::fs::write_executable;

use super::super::test_support::{
    action_context, fake_daemon_tool_root, known_daemon_detection, test_install_paths,
};
use super::{
    is_systemd_unit_inactive, stop_active_daemon, stop_active_daemon_with_detection,
    stop_active_daemon_with_quiescence, systemd_stop_error_is_satisfied_by_state,
};

#[test]
fn stop_active_daemon_errors_for_unmanaged_owner() {
    let detection = Detection {
        owner: Some(OwnerInfo {
            unique_name: None,
            pid: None,
            comm: Some("unknown-daemon".to_string()),
        }),
        daemons: Vec::new(),
    };
    let paths = test_install_paths();
    let (tx, _rx) = mpsc::sync_channel::<UiMessage>(4);
    let mut context = action_context(&paths, tx);

    let error = stop_active_daemon_with_detection(&mut context, &detection)
        .expect_err("unmanaged owner must block install");

    assert!(error.to_string().contains("not managed by a known unit"));
}

#[test]
fn stop_active_daemon_refreshes_ownership_after_the_initial_snapshot() {
    let root = fake_daemon_tool_root("fresh-owner-before-stop");
    write_executable(
        &root.join("busctl"),
        "#!/bin/sh\ncase \"$*\" in *NameHasOwner*) printf 'b true\\n' ;; *GetNameOwner*) printf 's \":1.99\"\\n' ;; *'status :1.99'*) printf 'Comm=appeared-daemon\\n' ;; *) exit 1 ;; esac\n",
    );
    let _tools = crate::system_tools::routing::use_fake_tool_bin(&root);
    let paths = test_install_paths();
    let (tx, _rx) = mpsc::sync_channel::<UiMessage>(8);
    let mut context = action_context(&paths, tx);

    let error = stop_active_daemon(&mut context)
        .expect_err("a daemon appearing after initial detection must block install");

    assert!(
        error.to_string().contains("not managed by a known unit"),
        "unexpected fresh-owner error: {error:#}"
    );
    std::fs::remove_dir_all(root).expect("remove fresh owner fixture");
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
    wait_for_child_program(&mut child, "sleep");
    let detection = Detection {
        owner: Some(OwnerInfo {
            unique_name: None,
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
    let mut context = action_context(&paths, tx);

    stop_active_daemon_with_detection(&mut context, &detection)
        .expect("stable process stop should succeed");

    let status = wait_for_child_exit(&mut child);
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
    let mut context = action_context(&paths, tx);

    stop_active_daemon_with_detection(&mut context, &detection)
        .expect("reinstall stop should succeed");

    let calls = std::fs::read_to_string(&calls).expect("systemctl calls");
    assert_eq!(calls.trim(), "--user stop unixnotis-daemon.service");
    let _cleanup = std::fs::remove_dir_all(root);
}

#[test]
fn stop_does_not_report_success_when_quiescence_check_still_fails() {
    let root = fake_daemon_tool_root("stop-quiescence-required");
    write_executable(&root.join("systemctl"), "#!/bin/sh\nexit 0\n");
    let _commands = crate::service_manager::contract::command_routing::use_fake_command_bin(&root);
    let _tools = crate::system_tools::routing::use_fake_tool_bin(&root);
    let detection = known_daemon_detection("unixnotis-daemon", true, Vec::new());
    let paths = test_install_paths();
    let (tx, _rx) = mpsc::sync_channel::<UiMessage>(8);
    let mut context = action_context(&paths, tx);

    let error = stop_active_daemon_with_quiescence(&mut context, &detection, |_paths| {
        Err(anyhow::anyhow!("runtime is still live"))
    })
    .expect_err("successful stop command must not bypass a live runtime");

    assert!(format!("{error:#}").contains("runtime is still live"));
    std::fs::remove_dir_all(root).expect("remove stop quiescence fixture");
}

#[test]
fn stop_command_failure_is_accepted_only_after_runtime_quiescence() {
    let root = fake_daemon_tool_root("stop-command-stale-failure");
    write_executable(&root.join("systemctl"), "#!/bin/sh\nexit 1\n");
    let _commands = crate::service_manager::contract::command_routing::use_fake_command_bin(&root);
    let _tools = crate::system_tools::routing::use_fake_tool_bin(&root);
    let detection = known_daemon_detection("unixnotis-daemon", true, Vec::new());
    let paths = test_install_paths();
    let (tx, _rx) = mpsc::sync_channel::<UiMessage>(8);
    let mut context = action_context(&paths, tx);

    stop_active_daemon_with_quiescence(&mut context, &detection, |_paths| Ok(()))
        .expect("a stale stop failure is safe after runtime quiescence");

    std::fs::remove_dir_all(root).expect("remove stale stop fixture");
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
    let root = fake_daemon_tool_root("systemctl-state");
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

    let _cleanup = std::fs::remove_dir_all(root);
}

fn wait_for_child_exit(child: &mut std::process::Child) -> std::process::ExitStatus {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        if let Some(status) = child.try_wait().expect("inspect stopped sleep daemon") {
            return status;
        }
        if std::time::Instant::now() >= deadline {
            let _kill = child.kill();
            let _reaped = child.wait();
            panic!("daemon stop did not terminate the expected process before deadline");
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

fn wait_for_child_program(child: &mut std::process::Child, expected: &str) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        if crate::detect::read_cmdline_program(child.id()).as_deref() == Some(expected) {
            return;
        }
        if std::time::Instant::now() >= deadline {
            let _kill = child.kill();
            let _reaped = child.wait();
            panic!("child did not enter expected program {expected} before deadline");
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}
