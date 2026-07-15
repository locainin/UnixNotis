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

pub mod assets;
pub mod config;
pub mod control;
pub mod css;
pub mod filesystem;
pub mod model;
pub mod reconnect;
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
