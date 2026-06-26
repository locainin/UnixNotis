//! Command-line options for installer startup
//!
//! This module is intentionally small and side-effect-light:
//! it only turns process arguments into an installer action.
//! The actual installer startup code can then decide whether to
//! print help or continue into the TUI/install flow

use std::env;
use std::ffi::OsString;

use anyhow::{anyhow, Result};

use crate::paths::ServiceManagerChoice;

/// Parsed command-line arguments that affect normal installer execution
///
/// Keep this type focused on options that still result in the installer
/// running. Actions that short-circuit startup, such as `--help`, belong in
/// `CliAction` instead
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CliArgs {
    /// Optional service-manager override selected by the user
    ///
    /// When this is `None`, the path/service discovery layer decides which
    /// backend to use, usually by falling back to the default backend
    pub service_manager: Option<ServiceManagerChoice>,
}

/// Top-level command-line result
///
/// Parsing can either produce arguments for a normal installer run or a
/// short-circuit action such as help output
#[derive(Debug)]
pub(crate) enum CliAction {
    /// Continue into installer startup using the parsed options
    Run(CliArgs),

    /// Print usage and exit before starting the TUI or doing detection work
    Help,
}

/// Parse arguments from the current process environment
///
/// This is the production entry point. Tests use `parse_args` directly so they
/// can provide exact argument vectors without depending on the test binary's
/// own command-line arguments
pub(crate) fn parse_env_args() -> Result<CliAction> {
    // Skip argv[0], which is the executable path/name and not an installer
    // option
    parse_args(env::args_os().skip(1))
}

/// Return the installer usage text
///
/// Keep this as a plain static string so help output is cheap and cannot fail
pub(crate) fn usage() -> &'static str {
    "Usage: unixnotis-installer [--service-manager systemd|dinit|runit|s6]\n"
}

/// Parse a sequence of OS-native argument strings
///
/// `OsString` is accepted at the boundary because process arguments are not
/// guaranteed to be valid UTF-8 on Unix. Individual options are converted to
/// UTF-8 only when this parser needs to match textual flags
fn parse_args<I>(args: I) -> Result<CliAction>
where
    I: IntoIterator<Item = OsString>,
{
    // Start with defaults. Any supported flag can override the relevant field
    let mut parsed = CliArgs {
        service_manager: None,
    };

    // Use a mutable iterator because some flags consume the next argument as a
    // value, for example `--service-manager runit`
    let mut args = args.into_iter();

    // Walk arguments left-to-right. Later service-manager flags intentionally
    // overwrite earlier ones, matching common CLI behavior for simple options
    while let Some(arg) = args.next() {
        // All currently supported flags are textual. Reject invalid UTF-8 early
        // so unknown non-UTF-8 options do not produce confusing behavior
        let text = arg
            .to_str()
            .ok_or_else(|| anyhow!("installer arguments must be valid UTF-8"))?;

        match text {
            // Help short-circuits normal startup. This avoids performing
            // detection, filesystem checks, or TUI initialization when the user
            // only wants usage text
            "-h" | "--help" => return Ok(CliAction::Help),

            // Support the split form:
            //
            //   --service-manager runit
            //
            // This consumes the following argument as the value
            "--service-manager" => {
                let value = args
                    .next()
                    .ok_or_else(|| anyhow!("--service-manager requires a value"))?;

                // Delegate value parsing so UTF-8 validation and backend-name
                // validation stay in one place
                parsed.service_manager = Some(parse_service_manager_arg(&value)?);
            }

            // Support the equals form:
            //
            //   --service-manager=runit
            //
            // This is convenient for scripts and mirrors many Unix CLIs
            _ if text.starts_with("--service-manager=") => {
                let value = text
                    .split_once('=')
                    .map(|(_, value)| value)
                    .unwrap_or_default();

                // `ServiceManagerChoice::parse` owns the accepted value list and
                // the error wording for unsupported managers
                parsed.service_manager = Some(ServiceManagerChoice::parse(value)?);
            }

            // Any other argument is currently unsupported. Failing closed keeps
            // typos visible instead of silently ignoring options the user
            // expected to matter
            _ => return Err(anyhow!("unsupported installer argument '{text}'")),
        }
    }

    // No short-circuit action was requested, so return the accumulated options
    // for normal installer execution
    Ok(CliAction::Run(parsed))
}

/// Parse the value passed to `--service-manager` in split-argument form
///
/// This helper exists because the split form still has an `OsString` value,
/// while the equals form already has a `&str` slice from the original flag
fn parse_service_manager_arg(value: &OsString) -> Result<ServiceManagerChoice> {
    // Service manager names are textual and intentionally restricted to valid
    // UTF-8. This keeps backend selection predictable across platforms
    let value = value
        .to_str()
        .ok_or_else(|| anyhow!("--service-manager value must be valid UTF-8"))?;

    // Keep accepted names centralized in `ServiceManagerChoice`
    ServiceManagerChoice::parse(value)
}

// CLI tests live beside this module instead of growing the production parser
#[cfg(test)]
#[path = "cli/tests.rs"]
mod tests;
