//! Session environment sync and shell startup helpers

mod shell_path;
mod sync;

pub use shell_path::{ensure_shell_path_entry, remove_shell_path_entry};
pub use sync::sync_user_environment;

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
