//! Shared service-manager artifacts, commands, probes, and refresh plans

mod artifact;
mod command;
// Fake service-manager routing lives under /tests and never enters production binaries
#[expect(
    clippy::cfg_not_test,
    reason = "production routing must not compile beside its test double"
)]
#[cfg(not(test))]
mod command_routing;
#[cfg(test)]
#[path = "tests/command_routing.rs"]
pub mod command_routing;
mod probe;
mod readiness;
mod refresh;
mod shell;

pub use artifact::{
    managed_directory_marker, managed_directory_marker_is_valid, ServiceArtifact,
    ServiceArtifactKind, MANAGED_DIRECTORY_MARKER, MANAGED_DIRECTORY_MARKER_CONTENTS,
};
pub use command::CommandSpec;
pub use probe::ServiceProbe;
pub use readiness::ReadinessIssue;
pub use refresh::{S6DatabaseRefresh, ServiceArtifactRefresh};
pub(super) use shell::{
    envdir_file_contents, envdir_sync_prelude, is_safe_env_name, render_envdir_shell_update,
    shell_quote, shell_quote_path,
};

#[cfg(test)]
mod tests;
