//! Command-line argument types for noticenterctl

mod args;
mod command;
mod dnd;

pub use args::{Args, DndState, DoctorServiceManagerArg, PresetCommand};
pub use args::{DebugLevelArg, InhibitScopeArg};
pub use command::Command;
pub use dnd::{DndClockTime, DndDuration};

#[cfg(test)]
mod tests;
