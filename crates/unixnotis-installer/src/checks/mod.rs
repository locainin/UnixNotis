//! Environment checks for session requirements and tooling availability

mod gtk;
mod output;
mod session;
mod shell;
mod system;

pub use session::{CheckItem, CheckState, Checks};
