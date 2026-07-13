//! Session environment sync and shell startup helpers

mod shell_path;
mod sync;

pub use shell_path::{ensure_shell_path_entry, remove_shell_path_entry};
pub use sync::sync_user_environment;
pub use sync::HYPR_IMPORT_VARS;

#[cfg(test)]
pub(in crate::actions::environment) use shell_path::{
    ensure_path_entry_in_file, format_path_for_shell_line, remove_path_entry_from_file,
    shell_path_entry_exists, shell_startup_files,
};

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
