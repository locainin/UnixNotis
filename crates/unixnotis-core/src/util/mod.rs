//! Shared helper utilities used across `UnixNotis` components

mod diagnostics;
mod display;
mod paths;
mod programs;

pub use diagnostics::{
    default_log_limit, diagnostic_log_limit, diagnostic_mode, log_limit, log_snippet,
};
pub use display::{
    fold_text_for_layout, sanitize_display_text, sanitize_display_text_bounded,
    sanitize_inline_display_text, sanitize_log_value, MAX_DISPLAY_TOKEN_WIDTH,
};
pub use paths::{expand_tilde, resolve_state_dir, resolve_state_dir_from_env, CONFIG_PATH_ENV};
pub use programs::{program_in_path, trusted_system_program_path, TRUSTED_SYSTEM_TOOL_DIRS};
