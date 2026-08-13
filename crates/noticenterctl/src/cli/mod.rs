//! Command-line argument types for noticenterctl

mod args;
mod command;
mod dnd;

pub use args::InhibitScopeArg;
pub use args::{
    Args, DevCommand, DndState, DoctorCommand, DoctorServiceManagerArg, PresetCommand, ThemeCommand,
};
pub use command::{Command, ExecutionKind};
pub use dnd::{DndClockTime, DndDuration};

#[cfg(test)]
mod tests;
