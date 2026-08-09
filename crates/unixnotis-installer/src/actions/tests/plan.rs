use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};

use crate::actions::ActionContext;
use crate::app::events::UiMessage;
use crate::model::ActionMode;
use crate::paths::InstallPaths;
use crate::service_manager::ServiceManager;

use super::{build_plan, run_step_with_reservation, steps_from_plan, StepKind};

#[test]
fn install_plan_stays_focused_on_build_and_install() {
    let plan = build_plan(ActionMode::Install);
    assert_eq!(
        plan,
        vec![
            StepKind::InstallCheck,
            StepKind::Build,
            StepKind::EnsureConfig,
            StepKind::StopDaemon,
            StepKind::InstallBinaries,
            StepKind::InstallService,
            StepKind::EnableService,
        ]
    );
}

#[test]
fn uninstall_plan_stops_daemon_before_removing_files() {
    let plan = build_plan(ActionMode::Uninstall);
    assert_eq!(
        plan,
        vec![
            StepKind::StopDaemon,
            StepKind::UninstallService,
            StepKind::RemoveBinaries,
            StepKind::RemoveState,
        ]
    );
}

#[test]
fn reset_plan_only_resets_config() {
    let plan = build_plan(ActionMode::Reset);
    assert_eq!(plan, vec![StepKind::ResetConfig]);
}

#[test]
fn test_plan_has_no_worker_steps() {
    // Test mode should leave the TUI idle instead of mutating the filesystem
    assert!(build_plan(ActionMode::Test).is_empty());
}

#[test]
fn steps_from_plan_uses_user_visible_labels() {
    let steps = steps_from_plan(&[
        StepKind::InstallCheck,
        StepKind::Build,
        StepKind::EnableService,
    ]);
    let labels = steps.into_iter().map(|step| step.name).collect::<Vec<_>>();
    assert_eq!(
        labels,
        vec![
            "Check existing install",
            "Prepare release binaries",
            "Enable user service"
        ]
    );
}

#[test]
fn restore_step_dispatches_to_validation_and_rejects_a_missing_backup() {
    let root = crate::test_support::fs::unique_temp_path("plan-restore-dispatch");
    let paths = InstallPaths {
        repo_root: root.join("repo"),
        bin_dir: root.join("bin"),
        service: ServiceManager::systemd_user(root.join("units")),
    };
    let (log_tx, _log_rx) = mpsc::sync_channel::<UiMessage>(4);
    let mut ctx = ActionContext {
        paths: &paths,
        install_state: None,
        log_tx,
        action_mode: ActionMode::Reset,
        restore_backup: None,
        service_reload_required: Arc::new(AtomicBool::new(false)),
    };

    let error = run_step_with_reservation(StepKind::RestoreConfig, &mut ctx, None)
        .expect_err("restore dispatch must preserve missing-backup validation");

    assert!(error.to_string().contains("no backup directory selected"));
}

#[test]
fn binary_install_rejects_calls_without_the_worker_owned_activation_guard() {
    let root = crate::test_support::fs::unique_temp_path("plan-guarded-binary-install");
    let paths = InstallPaths {
        repo_root: root.join("repo"),
        bin_dir: root.join("bin"),
        service: ServiceManager::systemd_user(root.join("units")),
    };
    let (log_tx, _log_rx) = mpsc::sync_channel::<UiMessage>(4);
    let mut ctx = ActionContext {
        paths: &paths,
        install_state: None,
        log_tx,
        action_mode: ActionMode::Install,
        restore_backup: None,
        service_reload_required: Arc::new(AtomicBool::new(false)),
    };

    let error = run_step_with_reservation(StepKind::InstallBinaries, &mut ctx, None)
        .expect_err("binary publication must require the worker-owned activation guard");

    assert!(error
        .to_string()
        .contains("binary installation requires daemon activation reservation"));
    assert!(
        !paths.bin_dir.exists(),
        "guard rejection must not mutate binaries"
    );
}
