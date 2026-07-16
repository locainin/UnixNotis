//! Command-line parsing for installer startup

mod model;
mod parse;

pub use model::{usage, version, CliAction, CliArgs};
pub use parse::parse_env_args;

#[cfg(test)]
mod tests;
