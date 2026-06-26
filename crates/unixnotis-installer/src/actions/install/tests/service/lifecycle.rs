use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

use crate::detect::Detection;
use crate::events::{UiMessage, WorkerEvent};
use crate::model::ActionMode;
use crate::service_manager::ServiceManager;

use super::super::super::service::{
    install_service, service_start_mode_from_enabled, uninstall_service, write_service_artifact,
    ServiceStartMode,
};
use super::super::support::{test_context, test_paths, test_root};
use super::expected_primary_artifact_contents;
use super::flow_support::{flow_env, write_fake_tools, FakeToolMode};

// Lifecycle tests assert the installer-visible behavior around reload flags and user logs
// Artifact byte tests live elsewhere so this file stays focused on install phase decisions

#[test]
fn install_service_skips_rewrite_when_unit_is_already_current() {
    let root = test_root("install-service-unchanged");
    let paths = test_paths(&root);
    fs::create_dir_all(paths.service.artifact_root()).expect("make service artifact dir");
    // Seed exactly what the backend would render so the installer should stay quiet
    let expected = expected_primary_artifact_contents(&paths);
    fs::write(paths.service.primary_artifact_path(), &expected).expect("write current artifact");

    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let mut ctx = test_context(&detection, &paths, ActionMode::Install);
    // Start as true so the test proves install_service actively clears stale reload state
    let reload_required = Arc::new(AtomicBool::new(true));
    ctx.service_reload_required = reload_required.clone();

    install_service(&mut ctx).expect("install service should succeed");

    // Existing bytes should be left unchanged, which keeps reloads and logs quiet
    assert_eq!(
        fs::read_to_string(paths.service.primary_artifact_path()).expect("read service artifact"),
        expected
    );
    assert!(!reload_required.load(Ordering::Acquire));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn install_service_marks_reload_when_unit_changes() {
    let root = test_root("install-service-changed");
    let paths = test_paths(&root);
    fs::create_dir_all(paths.service.artifact_root()).expect("make service artifact dir");
    // The old unit body simulates a real upgrade where manager state must be refreshed
    fs::write(
        paths.service.primary_artifact_path(),
        "[Unit]\nDescription=old\n",
    )
    .expect("write old service artifact");

    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let mut ctx = test_context(&detection, &paths, ActionMode::Install);
    let reload_required = Arc::new(AtomicBool::new(false));
    ctx.service_reload_required = reload_required.clone();

    install_service(&mut ctx).expect("install service should succeed");

    // A changed primary artifact should request the backend's reload or reload-equivalent path
    assert!(reload_required.load(Ordering::Acquire));
    assert_eq!(
        fs::read_to_string(paths.service.primary_artifact_path()).expect("read service artifact"),
        expected_primary_artifact_contents(&paths)
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn service_start_mode_uses_start_for_enabled_reinstalls() {
    // Reinstalls should not rerun enable when cached state already says enabled
    assert_eq!(
        service_start_mode_from_enabled(Some(true)),
        ServiceStartMode::StartOnly
    );
    assert_eq!(
        service_start_mode_from_enabled(Some(false)),
        ServiceStartMode::EnableAndStart
    );
    assert_eq!(
        service_start_mode_from_enabled(None),
        ServiceStartMode::EnableAndStart
    );
}

#[test]
fn install_service_reports_runit_unchanged_when_only_start_gate_is_recreated() {
    let root = test_root("install-service-runit-temp-gate");
    let mut paths = test_paths(&root);
    paths.service = ServiceManager::runit_user(root.join("home").join(".config").join("service"));
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let setup_ctx = test_context(&detection, &paths, ActionMode::Install);
    let steady_artifacts = paths.service.artifacts(&paths.bin_dir);
    for artifact in &steady_artifacts {
        // Seed the healthy installed state without the temporary runit down gate
        write_service_artifact(&setup_ctx, artifact).expect("steady runit artifact should exist");
    }

    let service_dir = paths.service.primary_artifact_path();
    let down_gate = service_dir.join("down");
    // The regression starts from healthy runit state, where down is absent after startup
    assert!(
        fs::symlink_metadata(&down_gate).is_err(),
        "healthy runit steady state should not keep the down gate"
    );

    // Capture the worker log so the regression checks user-visible wording too
    let (log_tx, log_rx) = mpsc::sync_channel::<UiMessage>(8);
    // Start true so recreating only the temporary gate must clear the reload flag
    let reload_required = Arc::new(AtomicBool::new(true));
    let mut ctx = test_context(&detection, &paths, ActionMode::Install);
    ctx.log_tx = log_tx;
    ctx.service_reload_required = reload_required.clone();

    // Reinstall creates down as a safety gate, but that gate is not part of steady state
    install_service(&mut ctx).expect("runit reinstall should recreate the start gate");

    assert!(
        !reload_required.load(Ordering::Acquire),
        "temporary runit start gates should not request reloads"
    );
    assert!(
        down_gate.is_file(),
        "runit reinstall should still create the start gate before env sync"
    );
    let logs = log_rx.try_iter().collect::<Vec<_>>();
    // A temporary safety gate is not a meaningful service artifact change
    assert!(
        logs.iter().any(|event| matches!(
            event,
            UiMessage::Worker(WorkerEvent::LogLine(line))
                if line == "runit service directory already up to date"
        )),
        "temporary runit start gate writes should not log a service reinstall"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn uninstall_service_skips_removed_log_for_missing_runit_start_gate() {
    let root = test_root("uninstall-runit-missing-temp-gate");
    let fake_bin = root.join("fake-bin");
    let fake_log = root.join("fake-calls.log");
    write_fake_tools(&fake_bin, &fake_log, FakeToolMode::RunitSv);
    let _env = flow_env(&root, &fake_bin);

    let mut paths = test_paths(&root);
    paths.service = ServiceManager::runit_user(root.join("home").join(".config").join("service"));
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let setup_ctx = test_context(&detection, &paths, ActionMode::Install);
    for artifact in paths.service.artifacts(&paths.bin_dir) {
        // Steady runit state keeps the service directory and run script, but not down
        write_service_artifact(&setup_ctx, &artifact).expect("steady runit artifact should exist");
    }

    let service_dir = paths.service.primary_artifact_path();
    let down_gate = service_dir.join("down");
    assert!(
        fs::symlink_metadata(&down_gate).is_err(),
        "healthy runit service should not keep the temporary down gate"
    );

    let (log_tx, log_rx) = mpsc::sync_channel::<UiMessage>(16);
    let mut ctx = test_context(&detection, &paths, ActionMode::Uninstall);
    ctx.log_tx = log_tx;

    uninstall_service(&mut ctx).expect("runit uninstall should remove steady artifacts");

    let logs = log_rx.try_iter().collect::<Vec<_>>();
    assert!(
        !logs.iter().any(|event| matches!(
            event,
            UiMessage::Worker(WorkerEvent::LogLine(line))
                if line.contains("Removed runit service directory") && line.contains("/down")
        )),
        "missing temporary runit down gate should not be logged as removed"
    );
    assert!(
        logs.iter().any(|event| matches!(
            event,
            UiMessage::Worker(WorkerEvent::LogLine(line))
                if line.contains("Removed runit service directory")
                    && line.contains("unixnotis-daemon")
        )),
        "steady runit artifacts should still report real removals"
    );

    let _ = fs::remove_dir_all(&root);
}
