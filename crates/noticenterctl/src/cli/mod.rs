//! Command-line argument types for noticenterctl

mod args;
mod command;

pub(crate) use args::{Args, DndState, PresetCommand};
#[cfg(test)]
pub(crate) use args::{DebugLevelArg, InhibitScopeArg};
pub(crate) use command::Command;

#[cfg(test)]
mod tests;
