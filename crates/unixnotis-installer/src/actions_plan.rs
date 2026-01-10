//! Planning and dispatch for installer steps.
//!
//! Keeps the sequencing logic in one place so install, uninstall, and reset
//! flows stay predictable.

use anyhow::Result;

use crate::model::{ActionMode, ActionStep, StepStatus};

use super::{
    check_install_state_step, enable_service, ensure_config, install_binaries, install_service,
    remove_binaries, reset_config, run_build, run_verify, stop_active_daemon, uninstall_service,
    ActionContext,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepKind {
    InstallCheck,
    StopDaemon,
    Verify,
    Build,
    EnsureConfig,
    ResetConfig,
    InstallBinaries,
    InstallService,
    EnableService,
    UninstallService,
    RemoveBinaries,
}

pub fn build_plan(mode: ActionMode, verify: bool) -> Vec<StepKind> {
    match mode {
        ActionMode::Test => Vec::new(),
        ActionMode::Install => {
            let mut steps = vec![StepKind::InstallCheck];
            if verify {
                steps.push(StepKind::Verify);
            }
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
        ActionMode::Uninstall => vec![StepKind::UninstallService, StepKind::RemoveBinaries],
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
        StepKind::Verify => run_verify(ctx),
        StepKind::Build => run_build(ctx),
        StepKind::EnsureConfig => ensure_config(ctx),
        StepKind::ResetConfig => reset_config(ctx),
        StepKind::InstallBinaries => install_binaries(ctx),
        StepKind::InstallService => install_service(ctx),
        StepKind::EnableService => enable_service(ctx),
        StepKind::UninstallService => uninstall_service(ctx),
        StepKind::RemoveBinaries => remove_binaries(ctx),
    }
}

pub fn step_label(kind: StepKind) -> &'static str {
    match kind {
        StepKind::InstallCheck => "Check existing install",
        StepKind::StopDaemon => "Stop existing daemon",
        StepKind::Verify => "Verify workspace",
        StepKind::Build => "Build release binaries",
        StepKind::EnsureConfig => "Ensure config files",
        StepKind::ResetConfig => "Reset config files",
        StepKind::InstallBinaries => "Install binaries",
        StepKind::InstallService => "Install systemd unit",
        StepKind::EnableService => "Enable user service",
        StepKind::UninstallService => "Remove systemd unit",
        StepKind::RemoveBinaries => "Remove binaries",
    }
}
