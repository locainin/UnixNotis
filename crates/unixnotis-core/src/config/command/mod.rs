//! Command parsing and built-in widget command templates

pub(super) mod defaults;
mod parse;

pub use parse::{parse_command, CommandParseError, ExecutionMode, ParsedCommand};

#[cfg(test)]
mod tests;
