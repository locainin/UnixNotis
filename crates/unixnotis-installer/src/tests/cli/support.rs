use std::ffi::OsString;

use super::{CliAction, CliArgs};

/// Convenience helper for building test argument vectors
///
/// The parser accepts `OsString`, but most tests only need ordinary UTF-8
/// string literals
pub(super) fn args(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

pub(super) fn run_args(action: CliAction) -> CliArgs {
    let CliAction::Run(args) = action else {
        panic!("expected run action");
    };
    args
}
