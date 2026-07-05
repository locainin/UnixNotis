//! Command-line argument types for noticenterctl

mod args;
mod command;
mod debug;
mod dnd;
mod inhibit;
mod preset;

pub(crate) use args::Args;
pub(crate) use command::Command;
pub(crate) use debug::DebugLevelArg;
pub(crate) use dnd::DndState;
pub(crate) use inhibit::InhibitScopeArg;
pub(crate) use preset::PresetCommand;

#[cfg(test)]
mod tests;
