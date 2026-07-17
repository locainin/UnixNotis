//! Output formatting helpers for noticenterctl

mod error;
mod gate;
mod notifications;

pub use error::format_cli_error;
pub use gate::{allow_full_output, warn_full_requires_diagnostic};
pub use notifications::{print_inhibitors, print_notifications};
