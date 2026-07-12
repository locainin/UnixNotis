//! Shared types and configuration for UnixNotis.

#![allow(
    clippy::nursery,
    clippy::pedantic,
    reason = "pedantic and nursery cleanup is tracked incrementally across existing code"
)]

pub mod assets;
pub mod config;
pub mod control;
pub mod css;
pub mod model;
#[cfg(test)]
#[path = "tests/support.rs"]
mod test_support;
pub mod theme;
pub mod util;

pub use assets::*;
pub use config::*;
pub use control::*;
pub use css::*;
pub use model::*;
pub use theme::*;
pub use util::program_in_path;
