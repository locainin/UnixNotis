use crate::model::ActionMode;

use super::{build_plan, steps_from_plan, StepKind};

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
fn uninstall_plan_removes_service_before_binaries_and_state() {
    let plan = build_plan(ActionMode::Uninstall);
    assert_eq!(
        plan,
        vec![
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
    let steps = steps_from_plan(&[StepKind::InstallCheck, StepKind::EnableService]);
    let labels = steps.into_iter().map(|step| step.name).collect::<Vec<_>>();
    assert_eq!(
        labels,
        vec!["Check existing install", "Enable user service"]
    );
}
