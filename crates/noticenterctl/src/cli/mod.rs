//! Command-line argument types for noticenterctl

mod args;
mod command;

pub use args::{Args, DndState, PresetCommand};
#[cfg(test)]
pub use args::{DebugLevelArg, InhibitScopeArg};
pub use command::Command;

#[cfg(test)]
mod tests;
