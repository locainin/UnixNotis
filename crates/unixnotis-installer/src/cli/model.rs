//! Parsed installer actions and static command metadata

use crate::paths::ServiceManagerChoice;

/// Parsed command-line arguments that affect normal installer execution
///
/// Keep this type focused on options that still result in the installer
/// running. Actions that short-circuit startup belong in `CliAction`
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CliArgs {
    /// Optional service-manager override selected by the user
    ///
    /// When this is `None`, service discovery selects the default backend
    pub service_manager: Option<ServiceManagerChoice>,
}

/// Top-level command-line result
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CliAction {
    /// Continue into installer startup using the parsed options
    Run(CliArgs),
    /// Print usage and exit before starting the TUI
    Help,
    /// Print version and exit before starting the TUI
    Version,
}

/// Return the installer usage text
#[must_use]
pub const fn usage() -> &'static str {
    "Usage: unixnotis-installer [--service-manager systemd|dinit|runit|s6] [-h|--help] [-V|--version]\n"
}

/// Return the package version compiled into the installer
#[must_use]
pub const fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
