//! Installer action orchestration and shared exports.

#[path = "actions/actions_state/binaries.rs"]
mod binaries;
#[path = "actions/build/mod.rs"]
mod build;
#[path = "actions/config/mod.rs"]
mod config;
#[path = "actions/actions_state/conflicts.rs"]
mod conflicts;
#[path = "actions/actions_state/context.rs"]
mod context;
#[path = "actions/actions_state/daemon.rs"]
mod daemon;
#[path = "actions/environment/mod.rs"]
mod environment;
#[path = "actions/format/mod.rs"]
mod format;
#[path = "actions/hyprland/mod.rs"]
mod hyprland;
#[path = "actions/install/mod.rs"]
mod install;
#[path = "actions/actions_state/install_state.rs"]
mod install_state;
#[path = "actions/actions_state/plan.rs"]
mod plan;
#[path = "actions/actions_state/process.rs"]
mod process;
#[path = "actions/actions_state/state.rs"]
mod state;

pub use build::{
    detect_build_accel, detect_build_accel_without_repo, write_build_accel_config,
    BuildAccelConfigStatus, BuildAccelDetection, BuildAccelOutcome,
};
pub use context::ActionContext;
pub use format::{format_daemon_status, summarize_owner};
pub use install_state::{check_install_state, InstallState};
pub use plan::{build_plan, run_step, steps_from_plan, StepKind};

pub(super) use build::run_build;
pub(crate) use config::backup::{list_backup_dirs_for_ui, restore_config};
pub(super) use config::{ensure_config, remove_state, reset_config};
pub(super) use daemon::stop_active_daemon;
pub(super) use environment::{ensure_shell_path_entry, sync_user_environment, HYPR_IMPORT_VARS};
pub(super) use install::{
    enable_service, install_binaries, install_service, remove_binaries, uninstall_service,
};
pub(super) use process::{log_line, run_command, run_command_without_stdout};
pub(super) use state::check_install_state_step;
