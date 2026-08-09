use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;

use crate::actions::StepKind;
use crate::app::events::{UiMessage, WorkerEvent};
use crate::model::ActionMode;

use super::super::worker::{
    release_pending_after_completed_step, run_action_worker, InstallLifecycle,
};
use super::support::run_install_lifecycle_with_hooks;

#[test]
fn empty_worker_plan_still_reports_completion() {
    let root = crate::test_support::fs::unique_temp_path("empty-worker-plan");
    let paths = crate::paths::InstallPaths {
        repo_root: root.join("repo"),
        bin_dir: root.join("home").join(".local").join("bin"),
        service: crate::service_manager::ServiceManager::systemd_user(
            root.join("home")
                .join(".config")
                .join("systemd")
                .join("user"),
        ),
    };
    let (tx, rx) = mpsc::sync_channel(4);

    run_action_worker(&[], ActionMode::Install, &paths, None, None, &tx);

    assert!(matches!(
        rx.recv_timeout(std::time::Duration::from_secs(1))
            .expect("worker completion event"),
        UiMessage::Worker(WorkerEvent::Finished)
    ));
}

#[test]
fn worker_owned_guard_spans_install_steps_and_drops_before_controlled_start() {
    let alive = Arc::new(AtomicBool::new(false));
    let mut lifecycle = InstallLifecycle::new();

    run_install_lifecycle_with_hooks(
        &mut lifecycle,
        || Ok(()),
        || {
            Ok(crate::actions::DaemonActivationReservation::test_guard(
                Arc::clone(&alive),
            ))
        },
        |reservation| {
            assert!(alive.load(Ordering::Acquire));
            let _ = reservation;
            Ok(())
        },
        |reservation| {
            assert!(alive.load(Ordering::Acquire));
            let _ = reservation;
            Ok(())
        },
        |reservation| {
            assert!(alive.load(Ordering::Acquire));
            let _ = reservation;
            Ok(())
        },
        |reservation| {
            assert!(alive.load(Ordering::Acquire));
            let _ = reservation;
            Ok(())
        },
        || {
            assert!(!alive.load(Ordering::Acquire));
            Ok(())
        },
    )
    .expect("complete guarded install lifecycle");

    assert!(lifecycle.activation.is_none());
}

#[test]
fn release_rollback_state_starts_at_binary_activation_and_ends_after_readiness() {
    assert!(release_pending_after_completed_step(
        false,
        StepKind::InstallBinaries
    ));
    assert!(!release_pending_after_completed_step(
        true,
        StepKind::EnableService
    ));
    assert!(release_pending_after_completed_step(
        true,
        StepKind::InstallService
    ));
    assert!(!release_pending_after_completed_step(
        false,
        StepKind::EnsureConfig
    ));
}
