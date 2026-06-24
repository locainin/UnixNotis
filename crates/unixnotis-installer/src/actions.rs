//! Installer action orchestration and shared exports.

#[path = "actions/actions_binaries.rs"]
mod actions_binaries;
#[path = "actions/actions_daemon.rs"]
mod actions_daemon;
#[path = "actions/actions_plan.rs"]
mod actions_plan;
#[path = "actions/actions_process.rs"]
mod actions_process;
#[path = "actions/actions_state.rs"]
mod actions_state;
#[path = "actions/build/mod.rs"]
mod build;
#[path = "actions/config/mod.rs"]
mod config;
#[path = "actions/environment/mod.rs"]
mod environment;
#[path = "actions/format/mod.rs"]
mod format;
#[path = "actions/hyprland/mod.rs"]
mod hyprland;
#[path = "actions/install/mod.rs"]
mod install;

pub use actions_plan::{build_plan, run_step, steps_from_plan, StepKind};
pub use actions_state::{check_install_state, ActionContext, InstallState};
pub use build::{
    detect_build_accel, detect_build_accel_without_repo, write_build_accel_config,
    BuildAccelConfigStatus, BuildAccelDetection, BuildAccelOutcome,
};
pub use format::{format_daemon_status, summarize_owner};

pub(super) use actions_daemon::stop_active_daemon;
pub(super) use actions_process::{log_line, run_command, run_command_without_stdout};
pub(super) use actions_state::check_install_state_step;
pub(super) use build::run_build;
pub(crate) use config::backup::{list_backup_dirs_for_ui, restore_config};
pub(super) use config::{ensure_config, remove_state, reset_config};
pub(super) use environment::{ensure_shell_path_entry, sync_user_environment, HYPR_IMPORT_VARS};
pub(super) use install::{
    enable_service, install_binaries, install_service, remove_binaries, uninstall_service,
};
