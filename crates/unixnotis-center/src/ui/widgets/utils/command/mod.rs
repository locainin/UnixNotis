//! Command execution, scheduling, and parsing helpers for widgets

mod action;
mod capture;
mod command_exec;
mod command_parse;
mod command_queue;
mod plan;

pub(in crate::ui::widgets) use action::run_action_command_with_completion;
pub(in crate::ui::widgets) use capture::{
    run_command_capture_async, run_command_capture_status_async,
    run_command_capture_with_timeout_async,
};
pub(in crate::ui::widgets) use command_exec::kill_process_group;
pub(in crate::ui::widgets) use plan::{resolve_command_plan, CommandKind, CommandPlan};

pub use capture::configure_command_config_dir;
