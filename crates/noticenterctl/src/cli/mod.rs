//! Command-line argument types for noticenterctl

mod args;
mod route;

pub(crate) use args::{Args, DndState, PresetCommand};
#[cfg(test)]
pub(crate) use args::{DebugLevelArg, InhibitScopeArg};
pub(crate) use route::Command;

#[cfg(test)]
mod tests;
