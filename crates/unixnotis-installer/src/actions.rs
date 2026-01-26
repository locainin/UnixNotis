//! Installer action orchestration and shared exports.

#[path = "actions/actions_binaries.rs"]
mod actions_binaries;
#[path = "actions/actions_build_accel.rs"]
mod actions_build_accel;
#[path = "actions/actions_config.rs"]
mod actions_config;
#[path = "actions/actions_daemon.rs"]
mod actions_daemon;
#[path = "actions/actions_env.rs"]
mod actions_env;
#[path = "actions/actions_format.rs"]
mod actions_format;
#[path = "actions/actions_hyprland.rs"]
mod actions_hyprland;
#[path = "actions/actions_install.rs"]
mod actions_install;
#[path = "actions/actions_plan.rs"]
mod actions_plan;
#[path = "actions/actions_process.rs"]
mod actions_process;
#[path = "actions/actions_state.rs"]
mod actions_state;
#[path = "actions/actions_verify.rs"]
mod actions_verify;

pub use actions_build_accel::{
    detect_build_accel, detect_build_accel_without_repo, write_build_accel_config,
    BuildAccelConfigStatus, BuildAccelDetection, BuildAccelOutcome,
};
pub use actions_format::{format_daemon_status, summarize_owner};
pub use actions_plan::{build_plan, run_step, steps_from_plan, StepKind};
pub use actions_state::{check_install_state, ActionContext, InstallState};

pub(super) use actions_config::{ensure_config, remove_state, reset_config};
pub(super) use actions_daemon::stop_active_daemon;
pub(super) use actions_install::{
    enable_service, install_binaries, install_service, remove_binaries, uninstall_service,
};
pub(super) use actions_process::{log_line, run_command};
pub(super) use actions_state::check_install_state_step;
pub(super) use actions_verify::{run_build, run_verify};
