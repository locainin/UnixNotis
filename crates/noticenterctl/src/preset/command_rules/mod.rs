//! Portable command discovery, validation, and rewriting

mod checks;
mod collect;
mod model;
mod rewrite;
#[cfg(test)]
mod tests;
mod tokens;

pub use checks::{
    collect_host_specific_command_paths, collect_outside_command_paths,
    validate_command_paths_in_config_bytes, validate_config_command_paths_stay_in_root,
};
pub use collect::collect_command_references_from_config;
pub use model::{CommandReference, HostSpecificCommandPath, OutsideCommandPath};
pub use rewrite::rewrite_host_specific_command_paths;
pub(super) use tokens::resolve_command_path_token;
