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
mod plan;
mod process;
mod releases;
mod state;

pub use build::{
    detect_build_accel, detect_build_accel_without_repo, write_build_accel_config,
    BuildAccelConfigStatus, BuildAccelDetection, BuildAccelOutcome,
};
pub use context::ActionContext;
pub use daemon::ensure_selected_service_inactive;
pub use daemon::DaemonActivationReservation;
pub use format::{
    daemon_has_displayable_status, daemon_status_is_warning, format_daemon_status, summarize_owner,
};
pub use plan::run_step_with_reservation;
pub use plan::{build_plan, steps_from_plan, StepKind};

pub use build::run_build;
pub use config::backup::{list_backup_dirs_for_ui, restore_config};
pub use config::{ensure_config, remove_state, reset_config};
pub use daemon::stop_active_daemon;
pub use environment::{ensure_shell_path_entry, remove_shell_path_entry, sync_user_environment};
pub use install::{
    check_install_state, enforce_service_readiness, rollback_failed_activation, InstallState,
    InstallationDisposition, InstallerLock,
};
pub use install::{install_binaries, remove_binaries, uninstall_service};
pub use install::{
    install_service_under_reservation, prepare_service_start_under_reservation,
    restart_previous_service, rollback_pending_under_activation_reservation,
    start_service_and_verify,
};
pub use process::{log_line, run_command, run_command_without_stdout};
pub use releases::{commit_pending_release, pending_release_exists};
pub use state::check_install_state_step;
