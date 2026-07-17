//! Shared helper utilities used across `UnixNotis` components

mod commands;
mod diagnostics;
mod display;
mod paths;
mod programs;

pub use commands::{is_simple_command, SHELL_META_CHARS};
pub use diagnostics::{
    default_log_limit, diagnostic_log_limit, diagnostic_mode, log_limit, log_snippet,
};
pub use display::{
    sanitize_display_text, sanitize_display_text_bounded, sanitize_inline_display_text,
    sanitize_log_value,
};
pub use paths::{expand_tilde, resolve_state_dir, resolve_state_dir_from_env, CONFIG_PATH_ENV};
pub use programs::{program_in_path, trusted_system_program_path, TRUSTED_SYSTEM_TOOL_DIRS};
