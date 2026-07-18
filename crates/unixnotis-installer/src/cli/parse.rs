//! Side-effect-light parsing of process arguments

use std::env;
use std::ffi::OsString;

use anyhow::{anyhow, Result};

use super::{CliAction, CliArgs};
use crate::paths::ServiceManagerChoice;

/// Parse arguments from the current process environment
///
/// # Errors
///
/// Returns an error for unsupported, incomplete, or non-UTF-8 arguments
pub fn parse_env_args() -> Result<CliAction> {
    // The executable path in argv[0] is not an installer option
    parse_args(env::args_os().skip(1))
}

/// Parse a sequence of OS-native argument strings
///
/// `OsString` keeps the process boundary honest because Unix arguments do not
/// have to contain valid UTF-8
pub(super) fn parse_args<I>(args: I) -> Result<CliAction>
where
    I: IntoIterator<Item = OsString>,
{
    let mut parsed = CliArgs {
        service_manager: None,
    };
    let mut args = args.into_iter();

    // Options are processed left-to-right so the last backend override wins
    while let Some(arg) = args.next() {
        let text = arg
            .to_str()
            .ok_or_else(|| anyhow!("installer arguments must be valid UTF-8"))?;

        match text {
            "-h" | "--help" => return Ok(CliAction::Help),
            "-V" | "--version" => return Ok(CliAction::Version),
            "--service-manager" => {
                let value = args
                    .next()
                    .ok_or_else(|| anyhow!("--service-manager requires a value"))?;
                parsed.service_manager = Some(parse_service_manager_arg(&value)?);
            }
            _ if text.starts_with("--service-manager=") => {
                let value = text.split_once('=').map_or("", |(_prefix, value)| value);
                // Explicit empty values must not silently select a default
                parsed.service_manager = Some(ServiceManagerChoice::parse(value)?);
            }
            _ => return Err(anyhow!("unsupported installer argument '{text}'")),
        }
    }

    Ok(CliAction::Run(parsed))
}

fn parse_service_manager_arg(value: &OsString) -> Result<ServiceManagerChoice> {
    let value = value
        .to_str()
        .ok_or_else(|| anyhow!("--service-manager value must be valid UTF-8"))?;
    Ok(ServiceManagerChoice::parse(value)?)
}
