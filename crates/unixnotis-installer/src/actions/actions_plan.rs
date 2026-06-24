//! Planning and dispatch for installer steps.
//!
//! Keeps the sequencing logic in one place so install, uninstall, and reset
//! flows stay predictable.

use anyhow::Result;

use crate::model::{ActionMode, ActionStep, StepStatus};

use super::{
    check_install_state_step, enable_service, ensure_config, install_binaries, install_service,
    remove_binaries, remove_state, reset_config, restore_config, run_build, stop_active_daemon,
    uninstall_service, ActionContext,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepKind {
    InstallCheck,
    StopDaemon,
    Build,
    EnsureConfig,
    ResetConfig,
    RestoreConfig,
    InstallBinaries,
    InstallService,
    EnableService,
    UninstallService,
    RemoveBinaries,
    RemoveState,
}

pub fn build_plan(mode: ActionMode) -> Vec<StepKind> {
    match mode {
        ActionMode::Test => Vec::new(),
        ActionMode::Install => {
            // Install order keeps state checks and file placement predictable
            let mut steps = vec![StepKind::InstallCheck];
            steps.extend([
                StepKind::Build,
                StepKind::EnsureConfig,
                StepKind::StopDaemon,
                StepKind::InstallBinaries,
                StepKind::InstallService,
                StepKind::EnableService,
            ]);
            steps
        }
        ActionMode::Uninstall => vec![
            // Service is removed before deleting binaries and state files
            StepKind::UninstallService,
            StepKind::RemoveBinaries,
            StepKind::RemoveState,
        ],
        ActionMode::Reset => vec![StepKind::ResetConfig],
    }
}

pub fn steps_from_plan(plan: &[StepKind]) -> Vec<ActionStep> {
    plan.iter()
        .map(|kind| ActionStep {
            name: step_label(*kind),
            status: StepStatus::Pending,
        })
        .collect()
}

pub fn run_step(step: StepKind, ctx: &mut ActionContext) -> Result<()> {
    match step {
        StepKind::InstallCheck => check_install_state_step(ctx),
        StepKind::StopDaemon => stop_active_daemon(ctx),
        StepKind::Build => run_build(ctx),
        StepKind::EnsureConfig => ensure_config(ctx),
        StepKind::ResetConfig => reset_config(ctx),
        StepKind::RestoreConfig => restore_config(ctx),
        StepKind::InstallBinaries => install_binaries(ctx),
        StepKind::InstallService => install_service(ctx),
        StepKind::EnableService => enable_service(ctx),
        StepKind::UninstallService => uninstall_service(ctx),
        StepKind::RemoveBinaries => remove_binaries(ctx),
        StepKind::RemoveState => remove_state(ctx),
    }
}

pub fn step_label(kind: StepKind) -> &'static str {
    match kind {
        StepKind::InstallCheck => "Check existing install",
        StepKind::StopDaemon => "Stop existing daemon",
        StepKind::Build => "Build release binaries",
        StepKind::EnsureConfig => "Ensure config files",
        StepKind::ResetConfig => "Reset config files",
        StepKind::RestoreConfig => "Restore config backup",
        StepKind::InstallBinaries => "Install binaries",
        StepKind::InstallService => "Install service artifact",
        StepKind::EnableService => "Enable user service",
        StepKind::UninstallService => "Remove service artifact",
        StepKind::RemoveBinaries => "Remove binaries",
        StepKind::RemoveState => "Remove persisted state",
    }
}

#[cfg(test)]
mod tests {
    use crate::model::ActionMode;

    use super::{build_plan, StepKind};

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
}
