//! Command references and path findings shared by preset checks

use std::path::PathBuf;
use unixnotis_core::CommandSpec;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandReference {
    // Config field name used in inspect and warning output
    pub(crate) slot: String,
    // Typed command carried by the parsed config
    pub(crate) command: CommandSpec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutsideCommandPath {
    // Config slot that carried the outside path
    pub(crate) slot: String,
    // Typed command from the config
    pub(crate) command: CommandSpec,
    // Resolved first-token path used by the validator
    pub(crate) resolved_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostSpecificCommandPath {
    // Config slot that carried the host-specific path
    pub(crate) slot: String,
    // Typed command from the config
    pub(crate) command: CommandSpec,
    // Resolved first-token path under the config root
    pub(crate) resolved_path: PathBuf,
}

#[cfg(test)]
#[path = "tests/model.rs"]
mod tests;
