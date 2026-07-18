//! Command construction, output handling, and timeout execution

pub(super) mod builder;
mod output;
mod process;
mod runner;

pub(super) use builder::{build_command, set_command_config_dir};
pub(in crate::ui::widgets) use process::kill_process_group;
pub(super) use runner::{build_command_runtime, run_command_with_timeout};
