//! Typed child-process descriptions shared across `UnixNotis` binaries

mod legacy;
mod spec;

pub use legacy::{parse_legacy_command, LegacyCommandError};
pub use spec::CommandSpec;

#[cfg(test)]
mod tests;
