//! Shared types and configuration for `UnixNotis`

#![expect(
    clippy::assigning_clones,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::default_trait_access,
    clippy::format_push_string,
    clippy::needless_continue,
    clippy::option_if_let_else,
    clippy::or_fun_call,
    clippy::ref_option,
    clippy::struct_excessive_bools,
    reason = "reviewed compatibility, wire-format, and bounded numeric conversions that cannot change without breaking public configuration behavior"
)]

pub mod config;
pub mod control;
pub mod css;
pub mod embedded;
pub mod filesystem;
pub mod model;
pub mod reconnect;
#[cfg(test)]
#[path = "tests/support.rs"]
mod test_support;
pub mod util;

pub use config::*;
pub use control::*;
pub use css::*;
pub use embedded::*;
pub use model::*;
pub use util::program_in_path;

/// Compatibility path for script resources published before the embedded module was introduced
pub mod assets {
    pub use crate::embedded::scripts::*;
}

/// Compatibility path for CSS resources published before the embedded module was introduced
pub mod theme {
    pub use crate::embedded::css::*;
}
