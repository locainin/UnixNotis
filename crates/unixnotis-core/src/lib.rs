//! Shared types and configuration for UnixNotis.

#![allow(
    clippy::blanket_clippy_restriction_lints,
    clippy::nursery,
    clippy::pedantic,
    clippy::restriction,
    reason = "workspace clippy runs use these groups as review signals, not as zero-tolerance policy gates"
)]

pub mod config;
pub mod control;
pub mod css;
pub mod model;
pub mod theme;
pub mod util;

pub use config::*;
pub use control::*;
pub use css::*;
pub use model::*;
pub use theme::*;
pub use util::program_in_path;
