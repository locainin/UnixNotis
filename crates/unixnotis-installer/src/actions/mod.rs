//! Installer action orchestration and shared exports

mod binaries;
mod build;
mod config;
mod conflicts;
mod context;
mod daemon;
mod environment;
mod format;
mod hyprland;
mod install;
mod install_state;
mod plan;
mod process;
mod state;

pub use build::{
    detect_build_accel, detect_build_accel_without_repo, write_build_accel_config,
    BuildAccelConfigStatus, BuildAccelDetection, BuildAccelOutcome,
};
pub use context::ActionContext;
pub use format::{
    daemon_has_displayable_status, daemon_status_is_warning, format_daemon_status, summarize_owner,
};
pub use install_state::{check_install_state, InstallState};
pub use plan::{build_plan, run_step, steps_from_plan, StepKind};

pub use build::run_build;
pub use config::backup::{list_backup_dirs_for_ui, restore_config};
pub use config::{ensure_config, remove_state, reset_config};
pub use daemon::stop_active_daemon;
pub use environment::{
    ensure_shell_path_entry, remove_shell_path_entry, sync_user_environment, HYPR_IMPORT_VARS,
};
pub use install::{
    enable_service, install_binaries, install_service, remove_binaries, uninstall_service,
};
pub use process::{log_line, run_command, run_command_without_stdout};
pub use state::check_install_state_step;
