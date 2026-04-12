//! Shared command rules for preset config
//!
//! Import, export, inspect, and css-check all need the same command view:
//! what command slots exist, which ones point outside the config root,
//! and which ones can be rewritten into portable config-relative paths

#[path = "command_rules/checks.rs"]
mod checks;
#[path = "command_rules/collect.rs"]
mod collect;
#[path = "command_rules/rewrite.rs"]
mod rewrite;
#[cfg(test)]
#[path = "command_rules/tests.rs"]
mod tests;
#[path = "command_rules/tokens.rs"]
mod tokens;

use std::path::PathBuf;

pub(crate) use self::checks::{
    collect_host_specific_command_paths, collect_outside_command_paths,
    validate_command_paths_in_config_bytes, validate_config_command_paths_stay_in_root,
};
pub(crate) use self::collect::collect_command_references_from_config;
pub(crate) use self::rewrite::rewrite_host_specific_command_paths;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommandReference {
    // Config field name used in inspect and warning output
    pub(crate) slot: String,
    // Raw command string carried by the parsed config
    pub(crate) command: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OutsideCommandPath {
    // Config slot that carried the outside path
    pub(crate) slot: String,
    // Raw command string from the config
    pub(crate) command: String,
    // Resolved first-token path used by the validator
    pub(crate) resolved_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HostSpecificCommandPath {
    // Config slot that carried the host-specific path
    pub(crate) slot: String,
    // Raw command string from the config
    pub(crate) command: String,
    // Resolved first-token path under the config root
    pub(crate) resolved_path: PathBuf,
}
