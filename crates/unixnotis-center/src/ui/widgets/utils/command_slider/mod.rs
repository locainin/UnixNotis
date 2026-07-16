//! Command slider construction, action handling, polling, and state refresh

mod actions;
mod apply;
mod build;
mod gate;
mod layout;
mod poll;
mod refresh;
mod request;
mod schedule;
mod state;
mod value;
mod watching;
mod widget;

pub(super) use super::{CommandWatch, RefreshBackoff};
pub use widget::CommandSlider;

#[cfg(test)]
#[path = "tests/gate.rs"]
mod gate_tests;
#[cfg(test)]
#[path = "tests/layout.rs"]
mod layout_tests;
#[cfg(test)]
#[path = "tests/value.rs"]
mod value_tests;
